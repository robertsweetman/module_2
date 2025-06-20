use lambda_runtime::{service_fn, LambdaEvent, Error};
use serde::{Deserialize, Serialize};
use scraper::{Html, Selector};
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use std::env;
use reqwest::Client;
use aws_config;
use aws_sdk_s3::Client as S3Client;

use pdf_processing::{extract_codes, extract_text_from_pdf};

#[derive(Debug, Serialize, Deserialize, Clone)]
struct TenderRecord {
    title: String,
    resource_id: String,
    ca: String,
    info: String,
    published: String,
    deadline: String,
    procedure: String,
    status: String,
    pdf_url: String,
    awarddate: String,
    value: String,
    cycle: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Request {
    max_pages: Option<u32>,
    test_mode: Option<bool>,
    start_page: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Response {
    records_count: usize,
    success: bool,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    sample_records: Option<Vec<TenderRecord>>,
}

#[derive(Debug)]
enum StorageBackend {
    S3 { client: S3Client, bucket: String },
}

async fn read_codes_from_storage(
    storage: &StorageBackend,
    filename: &str,
) -> Result<Vec<String>, Error> {
    let content = match storage {
        StorageBackend::S3 { client, bucket } => {
            println!("Reading codes from S3: s3://{}/{}", bucket, filename);
            let response = client
                .get_object()
                .bucket(bucket)
                .key(filename)
                .send()
                .await
                .map_err(|e| format!("Failed to get object from S3: {}", e))?;

            let data = response
                .body
                .collect()
                .await
                .map_err(|e| format!("Failed to read S3 response body: {}", e))?;

            String::from_utf8(data.into_bytes().to_vec())
                .map_err(|e| format!("Failed to convert S3 data to string: {}", e))?
        }
    };

    // Parse codes using the same approach as pdf_processing
    let codes: Vec<String> = content
        .lines()
        .filter_map(|line| line.split(',').next())  // Take everything before first comma
        .map(|code| code.trim().to_string())
        .filter(|code| !code.is_empty())
        .collect();
    
    println!("Loaded {} codes from {}", codes.len(), filename);
    Ok(codes)
}

async fn function_handler(event: LambdaEvent<Request>) -> Result<Response, Error> {
    println!("Starting get_data Lambda...");

    // Flags from request
    let test_mode = event.payload.test_mode.unwrap_or(false);
    let start_page = event.payload.start_page.unwrap_or(1);
    let offset = event.payload.offset.unwrap_or(0);
    let max_pages = if test_mode { 1 } else { event.payload.max_pages.unwrap_or(10) };

    // Calculate page range
    let (actual_start, actual_end) = if offset > 0 { (1, offset + 1) } else { (start_page, start_page + max_pages) };

    // Setup HTTP client
    println!("Creating HTTP client ...");
    let client = Client::new();
    let base_url = "https://www.etenders.gov.ie/epps/quickSearchAction.do";

    // Setup DB connection (skip in test mode)
    let pool: Option<Pool<Postgres>> = if !test_mode {
        println!("Connecting to database...");
        let db_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&db_url)
            .await?;
        ensure_tender_table_exists(&pool).await?;
        ensure_pdf_table_exists(&pool).await?;
        Some(pool)
    } else {
        None
    };

    // Scrape tender pages
    println!("Scraping pages {}..{}", actual_start, actual_end - 1);
    let records = get_table_content(&client, base_url, actual_start, actual_end, test_mode).await?;
    println!("Fetched {} tender records", records.len());

    if let Some(pool_ref) = &pool {
        println!("Persisting tender records...");
        save_records(pool_ref, &records).await?;
        println!("Tender records saved");
    }

    // Load detection codes from S3
    let bucket_name = env::var("LAMBDA_BUCKET_NAME")
        .map_err(|_| "LAMBDA_BUCKET_NAME environment variable not set")?;
    
    // Initialize AWS S3 client
    let aws_config = aws_config::defaults(aws_config::BehaviorVersion::latest()).load().await;
    let s3_client = S3Client::new(&aws_config);
    let storage = StorageBackend::S3 { 
        client: s3_client, 
        bucket: bucket_name 
    };
    
    // Read codes from S3
    let codes = read_codes_from_storage(&storage, "codes.txt").await
        .map_err(|e| format!("Failed to read codes from S3: {}", e))?;
    
    if codes.len() > 0 {
        println!("First 5 codes: {:?}", &codes[..codes.len().min(5)]);
    } else {
        println!("WARNING: No codes loaded from S3!");
    }

    // Process PDFs
    if let Some(pool_ref) = &pool {
        for record in records.iter().filter(|r| !r.pdf_url.is_empty()) {
            if let Err(e) = process_pdf(&client, pool_ref, record, &codes).await {
                println!("Error processing {}: {}", record.resource_id, e);
            }
        }
    }

    Ok(Response {
        records_count: records.len(),
        success: true,
        message: format!("Processed {} tender records", records.len()),
        sample_records: if test_mode { Some(records) } else { None },
    })
}

// ================= DB UTILITIES =================

async fn ensure_tender_table_exists(pool: &Pool<Postgres>) -> Result<(), Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS tender_records (
            id SERIAL PRIMARY KEY,
            title TEXT NOT NULL,
            resource_id TEXT NOT NULL UNIQUE,
            ca TEXT NOT NULL,
            info TEXT NOT NULL,
            published TEXT NOT NULL,
            deadline TEXT NOT NULL,
            procedure TEXT NOT NULL,
            status TEXT NOT NULL,
            pdf_url TEXT NOT NULL,
            awarddate TEXT NOT NULL,
            value TEXT NOT NULL,
            cycle TEXT NOT NULL,
            created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
        )
        "#
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn ensure_pdf_table_exists(pool: &Pool<Postgres>) -> Result<(), Error> {
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

async fn save_records(pool: &Pool<Postgres>, records: &[TenderRecord]) -> Result<(), Error> {
    for rec in records {
        sqlx::query(
            r#"
            INSERT INTO tender_records 
            (title, resource_id, ca, info, published, deadline, procedure, status, pdf_url, awarddate, value, cycle)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)
            ON CONFLICT (resource_id) DO UPDATE SET
                title = EXCLUDED.title,
                ca = EXCLUDED.ca,
                info = EXCLUDED.info,
                published = EXCLUDED.published,
                deadline = EXCLUDED.deadline,
                procedure = EXCLUDED.procedure,
                status = EXCLUDED.status,
                pdf_url = EXCLUDED.pdf_url,
                awarddate = EXCLUDED.awarddate,
                value = EXCLUDED.value,
                cycle = EXCLUDED.cycle
            "#
        )
        .bind(&rec.title)
        .bind(&rec.resource_id)
        .bind(&rec.ca)
        .bind(&rec.info)
        .bind(&rec.published)
        .bind(&rec.deadline)
        .bind(&rec.procedure)
        .bind(&rec.status)
        .bind(&rec.pdf_url)
        .bind(&rec.awarddate)
        .bind(&rec.value)
        .bind(&rec.cycle)
        .execute(pool)
        .await?;
    }
    Ok(())
}

async fn store_pdf_content_with_codes(
    pool: &Pool<Postgres>,
    resource_id: &str,
    pdf_text: &str,
    detected_codes: &[String],
) -> Result<(), Error> {
    sqlx::query(
        r#"
        INSERT INTO pdf_content (resource_id, pdf_text, extraction_timestamp, processing_status, detected_codes, codes_count)
        VALUES ($1,$2,CURRENT_TIMESTAMP,'COMPLETED',$3,$4)
        ON CONFLICT (resource_id) DO UPDATE SET
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

// ================= PDF PROCESSING =================

async fn process_pdf(
    client: &Client,
    pool: &Pool<Postgres>,
    record: &TenderRecord,
    codes: &[String],
) -> Result<(), Error> {
    println!("Downloading PDF for {}", record.resource_id);
    let response = client.get(&record.pdf_url).send().await?;
    let response = response.error_for_status()?;
    let pdf_bytes = response.bytes().await?;

    let pdf_text = extract_text_from_pdf(&pdf_bytes).map_err(|e| {
        let err: Error = format!("Text extraction failed: {}", e).into();
        err
    })?;

    println!("Extracted {} characters from PDF {}", pdf_text.len(), record.resource_id);
    
    let detected_codes = extract_codes(&pdf_text, codes);
    println!("Detected {} codes in PDF {}: {:?}", detected_codes.len(), record.resource_id, detected_codes);
    
    // Debug: Show a sample of the PDF text to help with troubleshooting
    if pdf_text.len() > 200 {
        println!("PDF text sample: '{}'", &pdf_text[..200]);
    } else {
        println!("Full PDF text: '{}'", pdf_text);
    }
    
    store_pdf_content_with_codes(pool, &record.resource_id, &pdf_text, &detected_codes).await?;
    Ok(())
}

// ================= SCRAPER =================

async fn get_table_content(
    client: &Client,
    base_url: &str,
    start_page: u32,
    end_page: u32,
    test_mode: bool,
) -> Result<Vec<TenderRecord>, Error> {
    let mut out = Vec::new();

    for page in start_page..end_page {
        println!("Fetching page {}/{}", page, end_page - 1);
        let url = format!("{}?d-3680175-p={}&searchType=cftFTS&latest=true", base_url, page);
        let body = client.get(&url).send().await?.text().await?;
        let doc = Html::parse_document(&body);
        let row_sel = Selector::parse("tbody tr").unwrap();

        for row in doc.select(&row_sel) {
            let title_sel = Selector::parse("td:nth-child(2)").unwrap();
            let id_sel = Selector::parse("td:nth-child(3)").unwrap();
            let ca_sel = Selector::parse("td:nth-child(4)").unwrap();
            let pub_sel = Selector::parse("td:nth-child(6)").unwrap();
            let deadline_sel = Selector::parse("td:nth-child(7)").unwrap();
            let proc_sel = Selector::parse("td:nth-child(8)").unwrap();
            let status_sel = Selector::parse("td:nth-child(9)").unwrap();
            let pdf_sel = Selector::parse("td:nth-child(10)").unwrap();
            let award_sel = Selector::parse("td:nth-child(11)").unwrap();
            let value_sel = Selector::parse("td:nth-child(12)").unwrap();

            let resource_id = row.select(&id_sel).next().map(|n| n.inner_html().trim().to_string()).unwrap_or_default();
            let pdf_column = row.select(&pdf_sel).next().map(|n| n.inner_html().trim().to_string()).unwrap_or_default();
            let pdf_url = if !pdf_column.is_empty() {
                format!("https://www.etenders.gov.ie/epps/cft/downloadNoticeForAdvSearch.do?resourceId={}", resource_id)
            } else { String::new() };

            out.push(TenderRecord {
                title: row.select(&title_sel).next().map(|n| n.inner_html().trim().to_string()).unwrap_or_default(),
                resource_id,
                ca: row.select(&ca_sel).next().map(|n| n.inner_html().trim().to_string()).unwrap_or_default(),
                info: String::new(),
                published: row.select(&pub_sel).next().map(|n| n.inner_html().trim().to_string()).unwrap_or_default(),
                deadline: row.select(&deadline_sel).next().map(|n| n.inner_html().trim().to_string()).unwrap_or_default(),
                procedure: row.select(&proc_sel).next().map(|n| n.inner_html().trim().to_string()).unwrap_or_default(),
                status: row.select(&status_sel).next().map(|n| n.inner_html().trim().to_string()).unwrap_or_default(),
                pdf_url,
                awarddate: row.select(&award_sel).next().map(|n| n.inner_html().trim().to_string()).unwrap_or_default(),
                value: row.select(&value_sel).next().map(|n| n.inner_html().trim().to_string()).unwrap_or_default(),
                cycle: String::new(),
            });
        }

        if test_mode && out.len() >= 5 {
            break;
        }
    }

    Ok(out)
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    lambda_runtime::run(service_fn(function_handler)).await
} 