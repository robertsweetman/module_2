use lambda_runtime::{service_fn, LambdaEvent, Error, run};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use std::env;
use std::fs;
use std::time::Duration;

// Import the function from the lib.rs file
use pdf_processing::{extract_codes, extract_text_from_pdf};

// Remove the static HTTP client entirely
// Remove the tracing stuff entirely

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

async fn function_handler(event: LambdaEvent<PdfProcessingRequest>) -> Result<Response, Error> {
    let resource_id = event.payload.resource_id;
    let pdf_url = event.payload.pdf_url;
    
    println!("Processing PDF for resource_id: {}", resource_id);

    if pdf_url.is_empty() {
        return Ok(Response {
            resource_id,
            success: false,
            message: "No PDF URL provided".to_string(),
            text_length: None,
        });
    }

    // Create fresh HTTP client for each invocation
    let http_client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    // Create fresh database pool for each invocation
    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let db_pool = PgPoolOptions::new()
        .max_connections(1) // Lambda only needs 1
        .acquire_timeout(Duration::from_secs(5))
        .connect(&db_url)
        .await?;

    // Download PDF using the fresh client
    println!("Downloading PDF from: {}", pdf_url);
    let pdf_bytes = match http_client.get(&pdf_url).send().await {
        Ok(response) => match response.error_for_status() {
            Ok(resp) => resp.bytes().await?,
            Err(e) => {
                return Ok(Response {
                    resource_id,
                    success: false,
                    message: format!("Failed to download PDF: HTTP {}", e.status().unwrap_or_default()),
                    text_length: None,
                });
            }
        },
        Err(e) => {
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
        Ok(text) => text,
        Err(e) => {
            return Ok(Response {
                resource_id,
                success: false,
                message: format!("Failed to extract text from PDF: {}", e),
                text_length: None,
            });
        }
    };
    
    // Load codes from file (this can stay as is)
    let codes_text = fs::read_to_string("codes.txt").unwrap_or_default();
    let codes: Vec<String> = codes_text
        .lines()
        .filter_map(|line| line.split(',').next())
        .map(|code| code.trim().to_string())
        .filter(|code| !code.is_empty())
        .collect();
    
    // Detect codes in the PDF text
    let detected_codes = extract_codes(&pdf_text, &codes);
    let codes_count = detected_codes.len();
    
    println!("Detected {} codes in PDF", codes_count);
    
    // Ensure table exists
    ensure_table_exists(&db_pool).await?;
    
    // Store in pdf_content table
    match store_pdf_content_with_codes(&db_pool, &resource_id, &pdf_text, &detected_codes).await {
        Ok(_) => {
            // Explicitly close the pool
            db_pool.close().await;
            
            Ok(Response {
                resource_id,
                success: true,
                message: "Successfully processed PDF".to_string(),
                text_length: Some(pdf_text.len()),
            })
        },
        Err(e) => {
            db_pool.close().await;
            
            Ok(Response {
                resource_id,
                success: false,
                message: format!("Failed to store PDF content: {}", e),
                text_length: Some(pdf_text.len()),
            })
        }
    }
}

// Keep your existing database functions as-is
async fn ensure_table_exists(pool: &Pool<Postgres>) -> Result<(), Error> {
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
) -> Result<(), Error> {
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
    // Remove all the tracing stuff
    run(service_fn(function_handler)).await
}