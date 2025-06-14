use lambda_runtime::{service_fn, LambdaEvent, Error};
use serde::{Deserialize, Serialize};
use scraper::{Html, Selector};
use sqlx::{Pool, postgres::{PgPoolOptions, Postgres}};
use std::env;
use reqwest::Client;
use tokio;
use serde_json;
use aws_config;
use aws_sdk_sfn;

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
    
    // Get test mode and max pages from request or default to 10
    let max_pages = if test_mode { 
        println!("Running in test mode - fetching only 1 page");
        1 
    } else { 
        event.payload.max_pages.unwrap_or(10) 
    };
    
    println!("Fetching {} pages of data...", max_pages);
    // Get records
    let records = get_table_content(&client, base_url, max_pages, test_mode).await?;
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

    // Add this environment variable check
    let disable_step_function = std::env::var("DISABLE_STEP_FUNCTION")
        .unwrap_or_else(|_| "false".to_string()) == "true";

    // Only run Step Function code if not disabled
    if !pdf_records.is_empty() && !test_mode && !disable_step_function {
        println!("Found {} records with PDFs, triggering processing workflow", pdf_records.len());
        
        // Initialize AWS SDK
        let config = aws_config::defaults(aws_config::BehaviorVersion::latest()).load().await;
        let sfn_client = aws_sdk_sfn::Client::new(&config);
        
        // Get Step Function ARN from environment variable
        let step_function_arn = env::var("PDF_PROCESSING_STEP_FUNCTION_ARN")
            .unwrap_or_else(|_| {
                println!("Warning: PDF_PROCESSING_STEP_FUNCTION_ARN not set, using placeholder");
                "arn:aws:states:REGION:ACCOUNT_ID:stateMachine:pdf-processing-workflow".to_string()
            });
        
        // Construct the input payload
        let payload = serde_json::json!({
            "records": pdf_records
        });
        
        // Start Step Function execution
        match sfn_client.start_execution()
            .state_machine_arn(step_function_arn)
            .input(payload.to_string())
            .send()
            .await {
                Ok(response) => {
                    println!("Successfully triggered PDF processing workflow, execution ARN: {}", 
                             response.execution_arn().to_string());
                },
                Err(e) => {
                    println!("Failed to trigger PDF processing workflow: {}", e);
                }
            }
    }
    
    // Only save if not in test mode
    if !test_mode {
        if let Some(pool_ref) = &pool {
            println!("Saving records to database...");
            save_records(pool_ref, &records).await?;
            println!("Records saved successfully");
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

async fn save_records(pool: &Pool<Postgres>, records: &[TenderRecord]) -> Result<(), Error> {
    for record in records {
        sqlx::query(
            r#"
            INSERT INTO tender_records 
            (title, resource_id, ca, info, published, deadline, procedure, status, pdf_url, awarddate, value, cycle)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
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
        .bind(&record.title)
        .bind(&record.resource_id)
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
        .execute(pool)
        .await?;
    }
    
    Ok(())
}

async fn get_table_content(
    client: &Client,
    base_url: &str,
    total_pages: u32,
    test_mode: bool,
) -> Result<Vec<TenderRecord>, Error> {
    let mut records = Vec::new();

    for page in 1..=total_pages {
        println!("Fetching page {}/{}", page, total_pages);
        let url = format!("{}?d-3680175-p={}&searchType=cftFTS&latest=true", base_url, page);
        let response = client.get(&url).send().await?;
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

            let record = TenderRecord {
                title: row
                    .select(&title_selector)
                    .next()
                    .map(|n| n.inner_html().trim().to_string())
                    .unwrap_or_default(),
                resource_id,
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

            page_records.push(record);
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

#[tokio::main]
async fn main() -> Result<(), Error> {
    lambda_runtime::run(service_fn(function_handler)).await
}