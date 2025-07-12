use lambda_runtime::{service_fn, LambdaEvent, Error, run};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use std::env;
use std::fs;
use std::time::Duration;
use aws_lambda_events::event::sqs::SqsEvent;
use serde_json;
use aws_config;
use aws_sdk_sqs::Client as SqsClient;
use aws_sdk_s3::Client as S3Client;

// Import the function from the lib.rs file
use pdf_processing::{extract_codes, extract_text_from_pdf};

// Track if this container has been used
// Removed: Unused after redesign

#[derive(Debug, Serialize, Deserialize)]
struct PdfProcessingRequest {
    resource_id: i64,  // Changed from String to i64 to match the JSON
    pdf_url: String,
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
    // Deserialize the message body into our request struct
    let PdfProcessingRequest { resource_id, pdf_url } = match serde_json::from_str::<PdfProcessingRequest>(body_str) {
        Ok(req) => {
            println!("Successfully parsed JSON: resource_id={}, pdf_url={}", req.resource_id, req.pdf_url);
            req
        },
        Err(e) => {
            println!("ERROR: Failed to parse JSON: {:?}", e);
            println!("Raw message body: {}", body_str);
            return Ok(Response {
                resource_id: String::new(),
                success: false,
                message: format!("Failed to parse SQS message JSON: {}", e),
                text_length: None,
            });
        }
    };
    
    // resource_id is already i64, no need to parse
    let resource_id_int = resource_id;
    
    println!("Fresh container processing PDF for resource_id: {}", resource_id_int);

    if pdf_url.is_empty() {
        return Ok(Response {
            resource_id: resource_id.to_string(),
            success: false,
            message: "No PDF URL provided".to_string(),
            text_length: None,
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
                resource_id,
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
    match store_pdf_content_with_codes(&db_pool, resource_id_int, &pdf_text, &detected_codes).await {
        Ok(_) => {
            println!("Successfully stored PDF content for resource_id: {}", resource_id_int);
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

            // Build success response
            let _response = Response {
                resource_id: resource_id.to_string(),
                success: true,
                message: "Successfully processed PDF".to_string(),
                text_length: Some(pdf_text.len()),
            };

            // Flush stdout then terminate the process with a success code.
            use std::io::Write;
            let _ = std::io::stdout().flush();
            std::process::exit(0);
            #[allow(unreachable_code)]
            Ok(_response)
        },
        Err(e) => {
            println!("CRITICAL ERROR: Failed to store PDF content for resource_id {}: {}", resource_id_int, e);
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