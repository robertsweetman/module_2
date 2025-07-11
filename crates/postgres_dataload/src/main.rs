use lambda_runtime::{service_fn, LambdaEvent, Error};
use serde::{Deserialize, Serialize};
use scraper::{Html, Selector};
use sqlx::{Pool, postgres::{PgPoolOptions, Postgres}};
use std::env;
use reqwest::Client;
use tokio;
use serde_json;
use aws_config;
use aws_sdk_sqs::Client as SqsClient;
use chrono::{NaiveDate, NaiveDateTime};
use bigdecimal::BigDecimal;
use regex::Regex;
use std::str::FromStr;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct TenderRecord {
    title: String,
    resource_id: i64,
    ca: String,
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
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct TenderRecordRaw {
    title: String,
    resource_id: i64,
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

async fn function_handler(event: LambdaEvent<Request>) -> Result<Response, Error> {
    println!("Starting function handler...");
    
    // Get database connection
    let test_mode = event.payload.test_mode.unwrap_or(false);
    println!("Test mode: {}", test_mode);
    
    let pool = if !test_mode {
        println!("Attempting to connect to database...");
        let db_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        println!("Database URL found, connecting...");
        
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&db_url)
            .await?;
        
        println!("Connected to database, ensuring table exists...");
        // Ensure table exists
        ensure_table_exists(&pool).await?;
        println!("Table check complete");
        Some(pool)
    } else {
        println!("Test mode: skipping database connection");
        None
    };
    
    // Setup HTTP client
    println!("Setting up HTTP client...");
    let client = Client::new();
    let base_url = "https://www.etenders.gov.ie/epps/quickSearchAction.do";
    
    // Get parameters from request
    let start_page = event.payload.start_page.unwrap_or(1);
    let offset = event.payload.offset.unwrap_or(0);
    let max_pages = if test_mode {
        1
    } else {
        event.payload.max_pages.unwrap_or(10)
    };

    // If offset is set, override start_page and max_pages to look back from page 1
    let (actual_start, actual_end) = if offset > 0 {
        (1, offset + 1)
    } else {
        (start_page, start_page + max_pages)
    };
    
    println!("Fetching {} pages of data...", max_pages);
    // Get records
    let records = get_table_content(&client, base_url, actual_start, actual_end, test_mode).await?;
    println!("Successfully fetched {} records", records.len());
    
    // After processing the records but before returning the Response
    // Filter records with non-empty PDF URLs for processing
    let pdf_records: Vec<serde_json::Value> = records.iter()
        .filter(|r| !r.pdf_url.is_empty())
        .map(|r| serde_json::json!({
            "resource_id": r.resource_id,
            "pdf_url": r.pdf_url
        }))
        .collect();

    // Only save if not in test mode
    if !test_mode {
        if let Some(pool_ref) = &pool {
            println!("Saving records to database...");
            save_records(pool_ref, &records).await?;
            println!("Records saved successfully");
        }
    }
    
    if !pdf_records.is_empty() && !test_mode {
        println!("Found {} records with PDFs, queuing for processing", pdf_records.len());

        // Initialize AWS SDK and SQS client
        let config = aws_config::defaults(aws_config::BehaviorVersion::latest()).load().await;
        let sqs_client = SqsClient::new(&config);

        // Fetch queue URL from the environment
        let queue_url = match env::var("PDF_PROCESSING_QUEUE_URL") {
            Ok(url) => url,
            Err(_) => {
                println!("Error: PDF_PROCESSING_QUEUE_URL environment variable not set; skipping SQS send.");
                String::new()
            }
        };

        if !queue_url.is_empty() {
            for record in pdf_records {
                // Each record is already a serde_json::Value containing resource_id and pdf_url
                let message_body = record.to_string();

                match sqs_client
                    .send_message()
                    .queue_url(&queue_url)
                    .message_body(message_body)
                    .send()
                    .await
                {
                    Ok(resp) => println!(
                        "Queued record {} (message ID: {})", 
                        record["resource_id"].as_i64().unwrap_or(0),
                        resp.message_id().unwrap_or_default()
                    ),
                    Err(e) => println!(
                        "Failed to queue record {}: {}", 
                        record["resource_id"].as_i64().unwrap_or(0),
                        e
                    ),
                }
            }
        }
    }
    
    println!("Function completed successfully");
    Ok(Response {
        records_count: records.len(),
        success: true,
        message: format!("Successfully scraped {} tender records", records.len()),
        sample_records: if test_mode {
            Some(records.clone())
        } else {
            None
        },
    })
}

async fn ensure_table_exists(pool: &Pool<Postgres>) -> Result<(), Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS tender_records (
            id SERIAL PRIMARY KEY,
            title TEXT NOT NULL,
            resource_id TEXT NOT NULL UNIQUE,
            ca TEXT NOT NULL,
            info TEXT NOT NULL,
            published TIMESTAMP WITHOUT TIME ZONE,
            deadline TIMESTAMP WITHOUT TIME ZONE,
            procedure TEXT NOT NULL,
            status TEXT NOT NULL,
            pdf_url TEXT NOT NULL,
            awarddate DATE,
            value DECIMAL(15,2),
            cycle TEXT NOT NULL,
            bid INTEGER DEFAULT NULL,
            created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
        )
        "#
    )
    .execute(pool)
    .await?;
    
    Ok(())
}

async fn save_records(pool: &Pool<Postgres>, records: &[TenderRecord]) -> Result<(), Error> {
    for record in records {
        sqlx::query(
            r#"
            INSERT INTO tender_records 
            (title, resource_id, ca, info, published, deadline, procedure, status, pdf_url, awarddate, value, cycle, bid)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
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
                -- Note: We don't update bid column to preserve existing labels
            "#
        )
        .bind(&record.title)
        .bind(record.resource_id)
        .bind(&record.ca)
        .bind(&record.info)
        .bind(&record.published)
        .bind(&record.deadline)
        .bind(&record.procedure)
        .bind(&record.status)
        .bind(&record.pdf_url)
        .bind(&record.awarddate)
        .bind(&record.value)
        .bind(&record.cycle)
        .bind(&record.bid)
        .execute(pool)
        .await?;
    }
    
    Ok(())
}

async fn get_table_content(
    client: &Client,
    base_url: &str,
    start_page: u32,
    end_page: u32,
    test_mode: bool,
) -> Result<Vec<TenderRecord>, Error> {
    let mut records = Vec::new();

    for page in start_page..end_page {
        println!("Fetching page {}/{}", page, end_page);
        let url = format!("{}?d-3680175-p={}&searchType=cftFTS&latest=true", base_url, page);
        let response = match client.get(&url).send().await {
            Ok(resp) => resp,
            Err(e) => {
                let error_msg = format!("HTTP request to {} failed: {:?}", url, e);
                println!("{}", error_msg);
                return Err(Box::new(e).into());
            }
        };
        let body = response.text().await?;

        let document = Html::parse_document(&body);
        let row_selector = Selector::parse("tbody tr").unwrap();

        let mut page_records = Vec::new();

        for row in document.select(&row_selector) {
            let title_selector = Selector::parse("td:nth-child(2)").unwrap();
            let resource_id_selector = Selector::parse("td:nth-child(3)").unwrap();
            let ca_selector = Selector::parse("td:nth-child(4)").unwrap();
            let published_selector = Selector::parse("td:nth-child(6)").unwrap();
            let deadline_selector = Selector::parse("td:nth-child(7)").unwrap();
            let procedure_selector = Selector::parse("td:nth-child(8)").unwrap();
            let status_selector = Selector::parse("td:nth-child(9)").unwrap();
            let pdf_selector = Selector::parse("td:nth-child(10)").unwrap();
            let awarddate_selector = Selector::parse("td:nth-child(11)").unwrap();
            let value_selector = Selector::parse("td:nth-child(12)").unwrap();

            let resource_id = row
                .select(&resource_id_selector)
                .next()
                .map(|n| n.inner_html().trim().to_string())
                .unwrap_or_default();

            let pdf_column_content = row
                .select(&pdf_selector)
                .next()
                .map(|el| el.inner_html().trim().to_string())
                .unwrap_or_default();

            let pdf_url = if !pdf_column_content.is_empty() {
                format!("https://www.etenders.gov.ie/epps/cft/downloadNoticeForAdvSearch.do?resourceId={}", resource_id)
            } else {
                String::new()
            };

            // First collect raw data
            let raw_record = TenderRecordRaw {
                title: row
                    .select(&title_selector)
                    .next()
                    .map(|n| n.text().collect::<Vec<_>>().join("").trim().to_string())
                    .unwrap_or_default(),
                resource_id: resource_id.parse::<i64>().unwrap_or(0),
                ca: row
                    .select(&ca_selector)
                    .next()
                    .map(|n| n.inner_html().trim().to_string())
                    .unwrap_or_default(),
                info: String::new(),
                published: row
                    .select(&published_selector)
                    .next()
                    .map(|n| n.inner_html().trim().to_string())
                    .unwrap_or_default(),
                deadline: row
                    .select(&deadline_selector)
                    .next()
                    .map(|n| n.inner_html().trim().to_string())
                    .unwrap_or_default(),
                procedure: row
                    .select(&procedure_selector)
                    .next()
                    .map(|n| n.inner_html().trim().to_string())
                    .unwrap_or_default(),
                status: row
                    .select(&status_selector)
                    .next()
                    .map(|n| n.inner_html().trim().to_string())
                    .unwrap_or_default(),
                pdf_url,
                awarddate: row
                    .select(&awarddate_selector)
                    .next()
                    .map(|n| n.inner_html().trim().to_string())
                    .unwrap_or_default(),
                value: row
                    .select(&value_selector)
                    .next()
                    .map(|n| n.inner_html().trim().to_string())
                    .unwrap_or_default(),
                cycle: String::new(),
            };

            // Convert to proper types
            page_records.push(TenderRecord::from(raw_record));
        }

        println!("Found {} records on page {}", page_records.len(), page);
        records.extend(page_records);

        // Optional: break early if you just want to test the structure
        if test_mode && records.len() >= 5 {
            println!("Test mode: stopping after 5 records");
            break;
        }
    }

    Ok(records)
}

// Utility functions for parsing dates and values from Irish tender data
fn parse_irish_date(date_str: &str) -> Option<NaiveDate> {
    if date_str.is_empty() {
        return None;
    }
    
    // Parse HTML scraping date format: "24/06/2025 17:24:53"
    if let Ok(date) = NaiveDate::parse_from_str(date_str, "%d/%m/%Y %H:%M:%S") {
        return Some(date);
    }
    
    // Fallback formats (just in case)
    let fallback_formats = [
        "%d/%m/%Y",                 // 25/12/2024
        "%d-%m-%Y",                 // 25-12-2024
        "%Y-%m-%d",                 // 2024-12-25 (ISO format)
    ];
    
    for format in &fallback_formats {
        if let Ok(date) = NaiveDate::parse_from_str(date_str, format) {
            return Some(date);
        }
    }
    
    println!("Warning: Could not parse date: '{}'", date_str);
    None
}

fn parse_irish_datetime(dt_str: &str) -> Option<NaiveDateTime> {
    if dt_str.is_empty() { return None; }
    NaiveDateTime::parse_from_str(dt_str, "%d/%m/%Y %H:%M:%S").ok()
}

fn parse_tender_value(value_str: &str) -> Option<BigDecimal> {
    if value_str.is_empty() {
        return None;
    }
    
    // Create regex for parsing monetary values
    let value_regex = Regex::new(r"[€£$]?[\d,]+\.?\d*").unwrap();
    
    if let Some(captures) = value_regex.find(value_str) {
        let clean_value = captures.as_str()
            .replace("€", "")
            .replace("£", "")
            .replace("$", "")
            .replace(",", "");
            
        if let Ok(decimal_value) = BigDecimal::from_str(&clean_value) {
            return Some(decimal_value);
        }
    }
    
    println!("Warning: Could not parse value: '{}'", value_str);
    None
}

impl From<TenderRecordRaw> for TenderRecord {
    fn from(raw: TenderRecordRaw) -> Self {
        Self {
            title: raw.title,
            resource_id: raw.resource_id,
            ca: raw.ca,
            info: raw.info,
            published: parse_irish_datetime(&raw.published),
            deadline: parse_irish_datetime(&raw.deadline),
            procedure: raw.procedure,
            status: raw.status,
            pdf_url: raw.pdf_url,
            awarddate: parse_irish_date(&raw.awarddate),
            value: parse_tender_value(&raw.value),
            cycle: raw.cycle,
            bid: None,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    lambda_runtime::run(service_fn(function_handler)).await
}