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

// Import the function from the lib.rs file
use pdf_processing::{extract_codes, extract_text_from_pdf};

// Track if this container has been used
// Removed: Unused after redesign

#[derive(Debug, Serialize, Deserialize)]
struct PdfProcessingRequest {
    resource_id: String,
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
    // Check if this container has been used before
    // Removed: Unused after redesign
    
    // Expect exactly one record per invocation (batch_size = 1)
    let sqs_records = &event.payload.records;
    if sqs_records.is_empty() {
        return Ok(Response {
            resource_id: String::new(),
            success: false,
            message: "No SQS records received".to_string(),
            text_length: None,
        });
    }

    let sqs_message = &sqs_records[0];
    let body_str = match &sqs_message.body {
        Some(b) => b,
        None => {
            return Ok(Response {
                resource_id: String::new(),
                success: false,
                message: "SQS message body missing".to_string(),
                text_length: None,
            });
        }
    };

    // Deserialize the message body into our request struct
    let PdfProcessingRequest { resource_id, pdf_url } = match serde_json::from_str::<PdfProcessingRequest>(body_str) {
        Ok(req) => req,
        Err(e) => {
            return Ok(Response {
                resource_id: String::new(),
                success: false,
                message: format!("Failed to parse SQS message JSON: {}", e),
                text_length: None,
            });
        }
    };
    
    println!("Fresh container processing PDF for resource_id: {}", resource_id);

    if pdf_url.is_empty() {
        return Ok(Response {
            resource_id,
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
    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
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
                    resource_id,
                    success: false,
                    message: format!("Failed to download PDF: HTTP {}", e.status().unwrap_or_default()),
                    text_length: None,
                });
            }
        },
        Err(e) => {
            let _ = db_pool.close().await;
            return Ok(Response {
                resource_id,
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
                resource_id,
                success: false,
                message: format!("Failed to extract text from PDF: {}", e),
                text_length: None,
            });
        }
    };
    
    // Load codes from file
    println!("Loading codes from file");
    let codes_text = fs::read_to_string("codes.txt").unwrap_or_default();
    let codes: Vec<String> = codes_text
        .lines()
        .filter_map(|line| line.split(',').next())
        .map(|code| code.trim().to_string())
        .filter(|code| !code.is_empty())
        .collect();
    
    println!("Loaded {} codes", codes.len());
    
    // Detect codes in the PDF text
    let detected_codes = extract_codes(&pdf_text, &codes);
    let codes_count = detected_codes.len();
    
    println!("Detected {} codes in PDF", codes_count);
    
    // Ensure table exists
    println!("Ensuring table exists");
    if let Err(e) = ensure_table_exists(&db_pool).await {
        let _ = db_pool.close().await;
        return Ok(Response {
            resource_id,
            success: false,
            message: format!("Failed to ensure table exists: {}", e),
            text_length: Some(pdf_text.len()),
        });
    }
    
    // Store in pdf_content table
    println!("Storing PDF content in database");
    match store_pdf_content_with_codes(&db_pool, &resource_id, &pdf_text, &detected_codes).await {
        Ok(_) => {
            println!("Successfully stored PDF content");
            let _ = db_pool.close().await;

            // Delete the SQS message because we intend to exit non-gracefully afterwards.
            if let Some(receipt_handle) = &sqs_message.receipt_handle {
                // build a fresh SQS client using the same config so we don't re-use across threads
                let sqs_client = SqsClient::new(&aws_config::defaults(aws_config::BehaviorVersion::latest()).load().await);
                if let Ok(queue_url) = env::var("PDF_PROCESSING_QUEUE_URL") {
                    let _ = sqs_client
                        .delete_message()
                        .queue_url(queue_url)
                        .receipt_handle(receipt_handle)
                        .send()
                        .await;
                }
            }

            // Build success response
            let _response = Response {
                resource_id,
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
            println!("Failed to store PDF content: {}", e);
            let _ = db_pool.close().await;
            Ok(Response {
                resource_id,
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
            resource_id TEXT PRIMARY KEY,
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
    resource_id: &str, 
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

#[tokio::main]
async fn main() -> Result<(), Error> {
    println!("Lambda starting up");
    run(service_fn(function_handler)).await
}