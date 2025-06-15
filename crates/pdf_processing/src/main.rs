use lambda_runtime::{service_fn, LambdaEvent, Error, run};
use once_cell::sync::Lazy;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use std::env;
use std::fs;
use std::time::Duration;
use tracing_subscriber::{Registry, layer::SubscriberExt};
use std::sync::Once;

// Import the function from the lib.rs file
use pdf_processing::{extract_codes, extract_text_from_pdf};

// --- Static clients for performance and stability ---
// Create a single, static HTTP client to be reused across all invocations.
static HTTP_CLIENT: Lazy<Client> = Lazy::new(Client::new);

// Create a static, lazy-initialized database pool.
// It will only be created on the first database call and then reused.
static DB_POOL: Lazy<Pool<Postgres>> = Lazy::new(|| {
    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    PgPoolOptions::new()
        .max_connections(10) // Increased for robustness
        .acquire_timeout(Duration::from_secs(5))
        .connect_lazy(&db_url)
        .expect("Failed to create lazy database pool")
});

static INIT_TRACING: Once = Once::new();

fn install_noop_subscriber() {
    INIT_TRACING.call_once(|| {
        // a bare registry does nothing but counts as "set"
        let subscriber = Registry::default();
        let _ = tracing::subscriber::set_global_default(subscriber);
    });
}

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

    // Download PDF using the static client
    println!("Downloading PDF from: {}", pdf_url);
    let pdf_bytes = match HTTP_CLIENT.get(&pdf_url).send().await {
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
    
    // Load codes from file
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
    
    // Store in pdf_content table using the static pool
    match store_pdf_content_with_codes(&DB_POOL, &resource_id, &pdf_text, &detected_codes).await {
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
    install_noop_subscriber();       
    ensure_table_exists(&DB_POOL).await?;
    run(service_fn(function_handler)).await
}