use lambda_runtime::{service_fn, LambdaEvent, Error, run};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use std::env;
use std::time::Duration;
use aws_lambda_events::event::sqs::SqsEvent;
use serde_json;
use aws_config;
use aws_sdk_sqs::Client as SqsClient;
use aws_sdk_s3::Client as S3Client;
use chrono::{NaiveDate, NaiveDateTime};
use bigdecimal::BigDecimal;

// Import the function from the lib.rs file
use pdf_processing::{extract_codes, extract_text_from_pdf};

// Track if this container has been used
// Removed: Unused after redesign

#[derive(Debug, Serialize, Deserialize, Clone)]
struct TenderRecord {
    title: String,
    resource_id: i64, // Back to i64 for consistency
    contracting_authority: String,
    info: String,
    published: Option<NaiveDateTime>,
    deadline: Option<NaiveDateTime>,
    procedure: String,
    status: String,
    pdf_url: String,
    awarddate: Option<NaiveDate>,
    value: Option<BigDecimal>,
    cycle: String,
    bid: Option<i32>, // 1 = bid, 0 = no bid, NULL = unlabeled
    // This will be added during PDF processing
    pdf_content: Option<String>,
    detected_codes: Option<Vec<String>>, // Added by pdf_processing - actual codes found
    codes_count: Option<i32>, // Added by pdf_processing - count of detected codes
    processing_stage: Option<String>, // e.g. "ml_prediction"
}

#[derive(Debug, Serialize, Deserialize)]
struct Response {
    resource_id: String,
    success: bool,
    message: String,
    text_length: Option<usize>,
}

async fn function_handler(event: LambdaEvent<SqsEvent>) -> Result<Response, Error> {
    println!("=== FUNCTION HANDLER STARTED ===");
    println!("Event received, processing SQS records...");
    
    // Check if this container has been used before
    // Removed: Unused after redesign
    
    // Expect exactly one record per invocation (batch_size = 1)
    let sqs_records = &event.payload.records;
    println!("Number of SQS records: {}", sqs_records.len());
    
    if sqs_records.is_empty() {
        println!("No SQS records found in event");
        return Ok(Response {
            resource_id: String::new(),
            success: false,
            message: "No SQS records received".to_string(),
            text_length: None,
        });
    }

    let sqs_message = &sqs_records[0];
    println!("Processing SQS message, checking body...");
    let body_str = match &sqs_message.body {
        Some(b) => {
            println!("SQS message body found, length: {}", b.len());
            println!("Message body preview: {}", &b[..b.len().min(100)]);
            b
        },
        None => {
            println!("ERROR: SQS message body is None");
            return Ok(Response {
                resource_id: String::new(),
                success: false,
                message: "SQS message body missing".to_string(),
                text_length: None,
            });
        }
    };

    println!("Attempting to parse JSON from SQS message body...");
    // Deserialize the message body into our TenderRecord struct
    let mut tender_record = match serde_json::from_str::<TenderRecord>(body_str) {
        Ok(record) => {
            println!("Successfully parsed TenderRecord: resource_id={}, title={}, pdf_url={}", 
                    record.resource_id, record.title, record.pdf_url);
            record
        },
        Err(e) => {
            println!("ERROR: Failed to parse TenderRecord JSON: {:?}", e);
            println!("Raw message body: {}", body_str);
            return Ok(Response {
                resource_id: String::new(),
                success: false,
                message: format!("Failed to parse SQS message JSON: {}", e),
                text_length: None,
            });
        }
    };
    
    let resource_id = tender_record.resource_id;
    let pdf_url = tender_record.pdf_url.clone();
    
    println!("Fresh container processing PDF for resource_id: {}", resource_id);

    if pdf_url.is_empty() {
        println!("No PDF URL provided - routing to AI Summary for title-only analysis");
        
        // Route to AI Summary for title-only analysis
        tender_record.pdf_content = Some(String::new()); // Empty PDF content
        tender_record.detected_codes = Some(vec![]); // No codes
        tender_record.codes_count = Some(0); // Zero codes
        tender_record.processing_stage = Some("ai_summary_title_only".to_string());
        
        if let Err(e) = forward_to_ai_summary(&tender_record).await {
            println!("WARNING: Failed to forward to AI Summary queue: {}", e);
            return Ok(Response {
                resource_id: resource_id.to_string(),
                success: false,
                message: format!("No PDF URL and failed to forward to AI Summary: {}", e),
                text_length: None,
            });
        }
        
        return Ok(Response {
            resource_id: resource_id.to_string(),
            success: true,
            message: "No PDF URL - routed to AI Summary for title-only analysis".to_string(),
            text_length: Some(0),
        });
    }

    // Create fresh HTTP client for each invocation
    println!("Creating HTTP client");
    let http_client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // Create fresh database pool for each invocation
    println!("Creating database connection");
    let db_url = match env::var("DATABASE_URL") {
        Ok(url) => {
            println!("DATABASE_URL found, length: {}", url.len());
            url
        },
        Err(e) => {
            println!("ERROR: DATABASE_URL not found: {:?}", e);
            return Ok(Response {
                resource_id: resource_id.to_string(),
                success: false,
                message: format!("DATABASE_URL environment variable not set: {:?}", e),
                text_length: None,
            });
        }
    };
    let db_pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&db_url)
        .await
        .map_err(|e| format!("Failed to connect to database: {}", e))?;

    // Download PDF using the fresh client
    println!("Downloading PDF from: {}", pdf_url);
    let pdf_bytes = match http_client.get(&pdf_url).send().await {
        Ok(response) => match response.error_for_status() {
            Ok(resp) => {
                println!("PDF download successful, getting bytes");
                resp.bytes().await.map_err(|e| format!("Failed to get PDF bytes: {}", e))?
            },
            Err(e) => {
                let _ = db_pool.close().await;
                return Ok(Response {
                    resource_id: resource_id.to_string(),
                    success: false,
                    message: format!("Failed to download PDF: HTTP {}", e.status().unwrap_or_default()),
                    text_length: None,
                });
            }
        },
        Err(e) => {
            let _ = db_pool.close().await;
            return Ok(Response {
                resource_id: resource_id.to_string(),
                success: false,
                message: format!("Failed to send request: {}", e),
                text_length: None,
            });
        }
    };
    
    // Extract text from PDF
    println!("Extracting text from PDF ({} bytes)", pdf_bytes.len());
    let pdf_text = match extract_text_from_pdf(&pdf_bytes) {
        Ok(text) => {
            println!("Text extraction successful, {} characters", text.len());
            text
        },
        Err(e) => {
            let _ = db_pool.close().await;
            return Ok(Response {
                resource_id: resource_id.to_string(),
                success: false,
                message: format!("Failed to extract text from PDF: {}", e),
                text_length: None,
            });
        }
    };
    
    // Load codes from embedded content (instead of file system)
    println!("Loading codes from S3");
    let codes = match load_codes_from_s3().await {
        Ok(codes) => {
            println!("Loaded {} codes from S3", codes.len());
            codes
        },
        Err(e) => {
            let _ = db_pool.close().await;
            return Ok(Response {
                resource_id: resource_id.to_string(),
                success: false,
                message: format!("Failed to load codes from S3: {}", e),
                text_length: Some(pdf_text.len()),
            });
        }
    };
    
    // Detect codes in the PDF text
    let detected_codes = extract_codes(&pdf_text, &codes);
    let codes_count = detected_codes.len();
    
    println!("Detected {} codes in PDF", codes_count);
    
    // Ensure table exists
    println!("Ensuring table exists");
    if let Err(e) = ensure_table_exists(&db_pool).await {
        let _ = db_pool.close().await;
        return Ok(Response {
            resource_id: resource_id.to_string(),
            success: false,
            message: format!("Failed to ensure table exists: {}", e),
            text_length: Some(pdf_text.len()),
        });
    }
    
    // Store in pdf_content table
    println!("Storing PDF content in database");
    match store_pdf_content_with_codes(&db_pool, resource_id, &pdf_text, &detected_codes).await {
        Ok(_) => {
            println!("Successfully stored PDF content for resource_id: {}", resource_id);
            let _ = db_pool.close().await;

            // Only delete SQS message AFTER successful database storage
            println!("Deleting SQS message after successful database storage");
            if let Some(receipt_handle) = &sqs_message.receipt_handle {
                // build a fresh SQS client using the same config so we don't re-use across threads
                let sqs_client = SqsClient::new(&aws_config::defaults(aws_config::BehaviorVersion::latest()).load().await);
                if let Ok(queue_url) = env::var("PDF_PROCESSING_QUEUE_URL") {
                    match sqs_client
                        .delete_message()
                        .queue_url(queue_url)
                        .receipt_handle(receipt_handle)
                        .send()
                        .await
                    {
                        Ok(_) => println!("SQS message deleted successfully"),
                        Err(e) => println!("WARNING: Failed to delete SQS message: {}", e),
                    }
                }
            }

            // Update tender record with PDF processing results
            tender_record.pdf_content = Some(pdf_text.clone());
            tender_record.detected_codes = Some(detected_codes.clone());
            tender_record.codes_count = Some(codes_count as i32);
            
            // INTELLIGENT ROUTING: Check PDF content quality to decide next step
            let pdf_content_length = pdf_text.trim().len();
            let min_pdf_threshold = 100; // Minimum characters for meaningful ML analysis
            
            if pdf_content_length < min_pdf_threshold {
                // Route directly to AI Summary for title-only analysis
                println!("PDF content too minimal ({} chars < {} threshold) - routing to AI Summary for title-only analysis", 
                         pdf_content_length, min_pdf_threshold);
                
                tender_record.processing_stage = Some("ai_summary_title_only".to_string());
                if let Err(e) = forward_to_ai_summary(&tender_record).await {
                    println!("WARNING: Failed to forward to AI Summary queue: {}", e);
                    // Don't fail the whole process if queue forwarding fails
                }
            } else {
                // Route to ML prediction first (has substantial PDF content)
                println!("PDF content substantial ({} chars >= {} threshold) - routing to ML prediction first", 
                         pdf_content_length, min_pdf_threshold);
                
                tender_record.processing_stage = Some("ml_prediction".to_string());
                if let Err(e) = forward_to_ml_prediction(&tender_record).await {
                    println!("WARNING: Failed to forward to ML prediction queue: {}", e);
                    // Don't fail the whole process if queue forwarding fails
                }
            }

            // Build success response
            let response = Response {
                resource_id: resource_id.to_string(),
                success: true,
                message: "Successfully processed PDF".to_string(),
                text_length: Some(pdf_text.len()),
            };

            // Return success normally instead of exiting
            println!("Lambda completed successfully, returning response");
            Ok(response)
        },
        Err(e) => {
            println!("CRITICAL ERROR: Failed to store PDF content for resource_id {}: {}", resource_id, e);
            let _ = db_pool.close().await;
            
            // DO NOT delete SQS message on database failure - let it retry
            println!("NOT deleting SQS message due to database storage failure - message will retry");
            
            Ok(Response {
                resource_id: resource_id.to_string(),
                success: false,
                message: format!("Failed to store PDF content: {}", e),
                text_length: Some(pdf_text.len()),
            })
        }
    }
}

async fn ensure_table_exists(pool: &Pool<Postgres>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS pdf_content (
            resource_id BIGINT PRIMARY KEY,
            pdf_text TEXT NOT NULL,
            extraction_timestamp TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
            processing_status TEXT NOT NULL,
            metadata JSONB DEFAULT '{}'::JSONB,
            detected_codes TEXT[],
            codes_count INTEGER DEFAULT 0
        )
        "#
    )
    .execute(pool)
    .await?;
    
    Ok(())
}

async fn store_pdf_content_with_codes(
    pool: &Pool<Postgres>, 
    resource_id: i64, 
    pdf_text: &str,
    detected_codes: &[String]
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    sqlx::query(
        r#"
        INSERT INTO pdf_content 
        (resource_id, pdf_text, extraction_timestamp, processing_status, detected_codes, codes_count)
        VALUES ($1, $2, CURRENT_TIMESTAMP, 'COMPLETED', $3, $4)
        ON CONFLICT (resource_id) 
        DO UPDATE SET 
            pdf_text = EXCLUDED.pdf_text,
            extraction_timestamp = EXCLUDED.extraction_timestamp,
            processing_status = EXCLUDED.processing_status,
            detected_codes = EXCLUDED.detected_codes,
            codes_count = EXCLUDED.codes_count
        "#
    )
    .bind(resource_id)
    .bind(pdf_text)
    .bind(detected_codes)
    .bind(detected_codes.len() as i32)
    .execute(pool)
    .await?;
    
    Ok(())
}

async fn load_codes_from_s3() -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    println!("Initializing AWS config for S3");
    let config = aws_config::defaults(aws_config::BehaviorVersion::latest()).load().await;
    let s3_client = S3Client::new(&config);
    
    // Get S3 bucket and key from environment variables
    let bucket = match env::var("LAMBDA_BUCKET") {
        Ok(b) => {
            println!("LAMBDA_BUCKET found: {}", b);
            b
        },
        Err(e) => {
            println!("ERROR: LAMBDA_BUCKET not found: {:?}", e);
            return Err("LAMBDA_BUCKET environment variable not set".into());
        }
    };
    let key = "codes.txt";
    
    println!("Fetching codes from s3://{}/{}", bucket, key);
    
    let response = s3_client
        .get_object()
        .bucket(bucket)
        .key(key)
        .send()
        .await?;
    
    let body = response.body.collect().await?;
    let codes_text = String::from_utf8(body.into_bytes().to_vec())?;
    
    let codes: Vec<String> = codes_text
        .lines()
        .filter_map(|line| line.split(',').next())
        .map(|code| code.trim().to_string())
        .filter(|code| !code.is_empty())
        .collect();
    
    Ok(codes)
}

async fn forward_to_ml_prediction(tender_record: &TenderRecord) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Forwarding tender record {} to ML prediction queue", tender_record.resource_id);
    
    // Get ML prediction queue URL
    let ml_queue_url = env::var("ML_PREDICTION_QUEUE_URL")
        .map_err(|_| "ML_PREDICTION_QUEUE_URL environment variable not set")?;
    
    // Initialize SQS client
    let config = aws_config::defaults(aws_config::BehaviorVersion::latest()).load().await;
    let sqs_client = SqsClient::new(&config);
    
    // Add processing stage marker
    let mut record_with_stage = serde_json::to_value(tender_record)?;
    record_with_stage["processing_stage"] = serde_json::Value::String("ml_prediction".to_string());
    let message_body = record_with_stage.to_string();
    
    // Send message
    match sqs_client
        .send_message()
        .queue_url(&ml_queue_url)
        .message_body(message_body)
        .send()
        .await
    {
        Ok(resp) => {
            println!("Successfully forwarded record {} to ML prediction queue (message ID: {})", 
                    tender_record.resource_id, 
                    resp.message_id().unwrap_or_default());
            Ok(())
        },
        Err(e) => {
            println!("Failed to forward record {} to ML prediction queue: {}", 
                    tender_record.resource_id, e);
            Err(Box::new(e))
        }
    }
}

async fn forward_to_ai_summary(tender_record: &TenderRecord) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Forwarding tender record {} to AI Summary queue for title-only analysis", tender_record.resource_id);
    
    // Get AI Summary queue URL
    let ai_queue_url = env::var("AI_SUMMARY_QUEUE_URL")
        .map_err(|_| "AI_SUMMARY_QUEUE_URL environment variable not set")?;
    
    // Initialize SQS client
    let config = aws_config::defaults(aws_config::BehaviorVersion::latest()).load().await;
    let sqs_client = SqsClient::new(&config);
    
    // Create AI Summary message format
    // This matches the AISummaryMessage struct expected by ai_summary lambda
    let ai_message = serde_json::json!({
        "resource_id": tender_record.resource_id.to_string(),
        "tender_title": tender_record.title,
        "ml_prediction": {
            "should_bid": false, // Default for title-only processing
            "confidence": 0.0,
            "reasoning": "Title-only analysis - no PDF content available",
            "feature_scores": {
                "codes_count_score": 0.0,
                "has_codes_score": 0.0,
                "title_length_score": 0.0,
                "ca_score": 0.0,
                "text_features_score": 0.0,
                "total_score": 0.0
            }
        },
        "pdf_content": tender_record.pdf_content.as_ref().unwrap_or(&String::new()).clone(),
        "priority": "NORMAL", // Title-only gets normal priority
        "timestamp": chrono::Utc::now().to_rfc3339()
    });
    
    let message_body = ai_message.to_string();
    
    // Send message
    match sqs_client
        .send_message()
        .queue_url(&ai_queue_url)
        .message_body(message_body)
        .send()
        .await
    {
        Ok(resp) => {
            println!("Successfully forwarded record {} to AI Summary queue (message ID: {})", 
                    tender_record.resource_id, 
                    resp.message_id().unwrap_or_default());
            Ok(())
        },
        Err(e) => {
            println!("Failed to forward record {} to AI Summary queue: {}", 
                    tender_record.resource_id, e);
            Err(Box::new(e))
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    println!("=== Lambda starting up ===");
    println!("Rust backtrace level: {:?}", env::var("RUST_BACKTRACE"));
    println!("Available environment variables:");
    for (key, value) in env::vars() {
        if key.contains("DATABASE") || key.contains("LAMBDA") || key.contains("QUEUE") {
            println!("  {}: {}", key, value);
        }
    }
    println!("=== Starting lambda runtime ===");
    run(service_fn(function_handler)).await
}