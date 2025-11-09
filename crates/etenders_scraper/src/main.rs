use anyhow::{Context, Result};
use aws_config;
use aws_sdk_sqs::Client as SqsClient;
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime};
use lambda_runtime::{service_fn, Error, LambdaEvent};
use regex::Regex;
use reqwest::Client;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::env;
use std::str::FromStr;
use tracing::{error, info, warn};

#[derive(Debug, Serialize, Deserialize, Clone)]
struct TenderRecord {
    title: String,
    resource_id: i64,
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
    bid: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct TenderRecordRaw {
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
}

#[derive(Debug, Serialize, Deserialize)]
struct Response {
    records_count: usize,
    success: bool,
    message: String,
    queued_to_sqs: usize,
}

async fn function_handler(event: LambdaEvent<Request>) -> Result<Response, Error> {
    info!("=== ETENDERS SCRAPER STARTED ===");

    let test_mode = event.payload.test_mode.unwrap_or(false);
    let start_page = event.payload.start_page.unwrap_or(1);
    let max_pages = if test_mode {
        1
    } else {
        event.payload.max_pages.unwrap_or(10)
    };

    info!(
        "Configuration: test_mode={}, start_page={}, max_pages={}",
        test_mode, start_page, max_pages
    );

    let client = Client::new();
    let base_url = "https://www.etenders.gov.ie/epps/quickSearchAction.do";

    info!(
        "Scraping pages {}-{}",
        start_page,
        start_page + max_pages - 1
    );
    let records = scrape_tenders(&client, base_url, start_page, start_page + max_pages)
        .await
        .map_err(|e| Error::from(format!("Failed to scrape tenders: {}", e).as_str()))?;

    info!("Successfully scraped {} tender records", records.len());

    let mut queued_count = 0;

    if !test_mode {
        // Initialize AWS SQS client
        let aws_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .load()
            .await;
        let sqs_client = SqsClient::new(&aws_config);

        // Get the processing queue URL
        let processing_queue_url = env::var("TENDER_PROCESSING_QUEUE_URL")
            .map_err(|_| Error::from("TENDER_PROCESSING_QUEUE_URL not set"))?;

        info!(
            "Sending {} records to SQS queue: {}",
            records.len(),
            processing_queue_url
        );

        // Send each record to SQS
        for record in records.iter() {
            let message_body = serde_json::to_string(record)
                .map_err(|e| Error::from(format!("Failed to serialize record: {}", e).as_str()))?;

            match sqs_client
                .send_message()
                .queue_url(&processing_queue_url)
                .message_body(message_body)
                .send()
                .await
            {
                Ok(resp) => {
                    info!(
                        "Queued tender {} (message ID: {})",
                        record.resource_id,
                        resp.message_id().unwrap_or_default()
                    );
                    queued_count += 1;
                }
                Err(e) => {
                    error!("Failed to queue tender {}: {}", record.resource_id, e);
                }
            }
        }

        info!("Successfully queued {} records to SQS", queued_count);
    } else {
        info!("Test mode: skipping SQS queue");
    }

    info!("=== ETENDERS SCRAPER COMPLETED ===");

    Ok(Response {
        records_count: records.len(),
        success: true,
        message: format!(
            "Scraped {} tenders, queued {} to SQS",
            records.len(),
            queued_count
        ),
        queued_to_sqs: queued_count,
    })
}

async fn scrape_tenders(
    client: &Client,
    base_url: &str,
    start_page: u32,
    end_page: u32,
) -> Result<Vec<TenderRecord>> {
    let mut all_records = Vec::new();

    for page in start_page..end_page {
        info!("Fetching page {}/{}", page, end_page - 1);

        let url = format!(
            "{}?d-3680175-p={}&searchType=cftFTS&latest=true",
            base_url, page
        );

        let response = client
            .get(&url)
            .send()
            .await
            .context(format!("Failed to fetch page {}", page))?;

        let body = response
            .text()
            .await
            .context(format!("Failed to read response body for page {}", page))?;

        let doc = Html::parse_document(&body);
        let row_sel = Selector::parse("tbody tr").unwrap();

        let mut page_records = Vec::new();

        for row in doc.select(&row_sel) {
            match parse_tender_row(&row) {
                Ok(record) => page_records.push(record),
                Err(e) => {
                    warn!("Failed to parse tender row: {}", e);
                    continue;
                }
            }
        }

        info!("Parsed {} records from page {}", page_records.len(), page);
        all_records.extend(page_records);
    }

    Ok(all_records)
}

fn parse_tender_row(row: &scraper::ElementRef) -> Result<TenderRecord> {
    let title_sel = Selector::parse("td:nth-child(2)").unwrap(); // Title column with anchor
    let id_sel = Selector::parse("td:nth-child(3)").unwrap(); // Resource ID column
    let ca_sel = Selector::parse("td:nth-child(4)").unwrap();
    let pub_sel = Selector::parse("td:nth-child(5)").unwrap();
    let deadline_sel = Selector::parse("td:nth-child(6)").unwrap();
    let proc_sel = Selector::parse("td:nth-child(7)").unwrap();
    let status_sel = Selector::parse("td:nth-child(8)").unwrap();
    let pdf_sel = Selector::parse("td:nth-child(9)").unwrap();
    let award_sel = Selector::parse("td:nth-child(11)").unwrap();
    let value_sel = Selector::parse("td:nth-child(12)").unwrap();

    let col2_content = row
        .select(&title_sel)
        .next()
        .map(|n| n.inner_html().trim().to_string())
        .unwrap_or_default();

    let col3_content = row
        .select(&id_sel)
        .next()
        .map(|n| n.inner_html().trim().to_string())
        .unwrap_or_default();

    let col2_text = row
        .select(&title_sel)
        .next()
        .map(|n| n.text().collect::<Vec<_>>().join("").trim().to_string())
        .unwrap_or_default();

    let pdf_column = row
        .select(&pdf_sel)
        .next()
        .map(|n| n.inner_html().trim().to_string())
        .unwrap_or_default();

    // Debug logging
    info!("Column 2 (Title) innerHTML: {}", col2_content);
    info!("Column 2 (Title) text: {}", col2_text);
    info!("Column 3 (Resource ID) innerHTML: {}", col3_content);
    info!("Column 9 (PDF) innerHTML: {}", pdf_column);

    // Extract resource_id from column 3 (plain text number)
    let resource_id = col3_content.clone();

    // Extract title from column 2's anchor tag text
    let title = col2_text.clone();

    info!("Extracted resource_id: {}", resource_id);
    info!("Extracted title: {}", title);

    let pdf_url = if !resource_id.is_empty() {
        format!(
            "https://www.etenders.gov.ie/epps/cft/downloadNoticeForAdvSearch.do?resourceId={}",
            resource_id
        )
    } else {
        String::new()
    };

    let raw_record = TenderRecordRaw {
        title,
        resource_id,
        ca: row
            .select(&ca_sel)
            .next()
            .map(|n| n.inner_html().trim().to_string())
            .unwrap_or_default(),
        info: String::new(),
        published: row
            .select(&pub_sel)
            .next()
            .map(|n| n.inner_html().trim().to_string())
            .unwrap_or_default(),
        deadline: row
            .select(&deadline_sel)
            .next()
            .map(|n| n.inner_html().trim().to_string())
            .unwrap_or_default(),
        procedure: row
            .select(&proc_sel)
            .next()
            .map(|n| n.inner_html().trim().to_string())
            .unwrap_or_default(),
        status: row
            .select(&status_sel)
            .next()
            .map(|n| n.inner_html().trim().to_string())
            .unwrap_or_default(),
        pdf_url,
        awarddate: row
            .select(&award_sel)
            .next()
            .map(|n| n.inner_html().trim().to_string())
            .unwrap_or_default(),
        value: row
            .select(&value_sel)
            .next()
            .map(|n| n.inner_html().trim().to_string())
            .unwrap_or_default(),
        cycle: String::new(),
    };

    Ok(TenderRecord::from(raw_record))
}

impl From<TenderRecordRaw> for TenderRecord {
    fn from(raw: TenderRecordRaw) -> Self {
        TenderRecord {
            title: raw.title,
            resource_id: raw.resource_id.parse::<i64>().unwrap_or(0),
            contracting_authority: raw.ca,
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

fn parse_irish_date(date_str: &str) -> Option<NaiveDate> {
    if date_str.is_empty() {
        return None;
    }

    if let Ok(date) = NaiveDate::parse_from_str(date_str, "%d/%m/%Y %H:%M:%S") {
        return Some(date);
    }

    let fallback_formats = ["%d/%m/%Y", "%d-%m-%Y", "%Y-%m-%d"];

    for format in &fallback_formats {
        if let Ok(date) = NaiveDate::parse_from_str(date_str, format) {
            return Some(date);
        }
    }

    None
}

fn parse_irish_datetime(dt_str: &str) -> Option<NaiveDateTime> {
    if dt_str.is_empty() {
        return None;
    }
    NaiveDateTime::parse_from_str(dt_str, "%d/%m/%Y %H:%M:%S").ok()
}

fn parse_tender_value(value_str: &str) -> Option<BigDecimal> {
    if value_str.is_empty() {
        return None;
    }

    let value_regex = Regex::new(r"[€£$]?[\d,]+\.?\d*").unwrap();

    if let Some(captures) = value_regex.find(value_str) {
        let clean_value = captures
            .as_str()
            .replace("€", "")
            .replace("£", "")
            .replace("$", "")
            .replace(",", "");

        if let Ok(decimal_value) = BigDecimal::from_str(&clean_value) {
            return Some(decimal_value);
        }
    }

    None
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .without_time()
        .init();

    lambda_runtime::run(service_fn(function_handler)).await
}
