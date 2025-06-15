use lambda_runtime::{service_fn, LambdaEvent, Error};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use std::env;
use reqwest::Client;
use tracing_subscriber;
use std::fs;

// Import the function from the lib.rs file
use pdf_processing::extract_text_from_pdf;
use pdf_processing::extract_codes;

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
    // Extract data from event
    let resource_id = event.payload.resource_id;
    let pdf_url = event.payload.pdf_url;
    
    tracing::info!("Processing PDF for resource_id: {}", resource_id);
    
    if pdf_url.is_empty() {
        return Ok(Response {
            resource_id: resource_id.clone(),
            success: false,
            message: "No PDF URL provided".to_string(),
            text_length: None,
        });
    }
    
    // Get database connection
    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await?;
    
    // Ensure table exists
    ensure_table_exists(&pool).await?;
    
    // Download PDF
    let client = Client::new();
    tracing::info!("Downloading PDF from: {}", pdf_url);
    
    let pdf_bytes = match client.get(&pdf_url).send().await {
        Ok(response) => {
            if !response.status().is_success() {
                return Ok(Response {
                    resource_id,
                    success: false,
                    message: format!("Failed to download PDF: HTTP {}", response.status()),
                    text_length: None,
                });
            }
            
            match response.bytes().await {
                Ok(bytes) => bytes,
                Err(e) => {
                    return Ok(Response {
                        resource_id,
                        success: false,
                        message: format!("Failed to read PDF bytes: {}", e),
                        text_length: None,
                    });
                }
            }
        },
        Err(e) => {
            return Ok(Response {
                resource_id,
                success: false,
                message: format!("Failed to download PDF: {}", e),
                text_length: None,
            });
        }
    };
    
    // Extract text from PDF
    tracing::info!("Extracting text from PDF ({} bytes)", pdf_bytes.len());
    
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
    
    // Load codes from file
    let codes_text = fs::read_to_string("codes.txt").unwrap_or_default();
    // Keep only the numeric code before the first comma on each line
    let codes: Vec<String> = codes_text
        .lines()
        .filter_map(|line| line.split(',').next())
        .map(|code| code.trim().to_string())
        .filter(|code| !code.is_empty())
        .collect();
    
    // Detect codes in the PDF text
    let detected_codes = extract_codes(&pdf_text, &codes);
    let codes_count = detected_codes.len();
    
    tracing::info!("Detected {} codes in PDF", codes_count);
    
    // Store in pdf_content table with codes
    match store_pdf_content_with_codes(&pool, &resource_id, &pdf_text, &detected_codes).await {
        Ok(_) => {
            Ok(Response {
                resource_id,
                success: true,
                message: "Successfully processed PDF".to_string(),
                text_length: Some(pdf_text.len()),
            })
        },
        Err(e) => {
            Ok(Response {
                resource_id,
                success: false,
                message: format!("Failed to store PDF content: {}", e),
                text_length: Some(pdf_text.len()),
            })
        }
    }
}

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
    lambda_runtime::run(service_fn(function_handler)).await
}