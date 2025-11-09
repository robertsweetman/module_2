use aws_config;
use aws_lambda_events::event::sqs::SqsEvent;
use aws_sdk_sqs::Client as SqsClient;
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime};
use lambda_runtime::{Error, LambdaEvent, service_fn};
use serde::{Deserialize, Serialize};
use serde_json;
use sqlx::{Pool, Postgres, postgres::PgPoolOptions};
use std::env;
use tracing::{error, info};

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

#[derive(Debug, Serialize, Deserialize)]
struct Response {
    records_processed: usize,
    records_saved: usize,
    records_queued: usize,
    success: bool,
    message: String,
}

async fn function_handler(event: LambdaEvent<SqsEvent>) -> Result<Response, Error> {
    info!("=== POSTGRES DATALOAD STARTED ===");
    info!("Received {} SQS records", event.payload.records.len());

    // Connect to database
    let db_url = env::var("DATABASE_URL")
        .map_err(|_| Error::from("DATABASE_URL environment variable not set"))?;

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await
        .map_err(|e| Error::from(format!("Failed to connect to database: {}", e).as_str()))?;

    info!("Connected to database");

    // Ensure tables exist
    ensure_tables_exist(&pool)
        .await
        .map_err(|e| Error::from(format!("Failed to ensure tables exist: {}", e).as_str()))?;

    // Parse tender records from SQS messages
    let mut tender_records = Vec::new();

    for record in event.payload.records {
        if let Some(body) = &record.body {
            match serde_json::from_str::<TenderRecord>(body) {
                Ok(tender) => {
                    info!("Parsed tender: {}", tender.resource_id);
                    tender_records.push(tender);
                }
                Err(e) => {
                    error!("Failed to parse SQS message body: {}", e);
                    continue;
                }
            }
        }
    }

    info!("Parsed {} tender records from SQS", tender_records.len());

    // Filter out duplicates (records already in database)
    let new_records = filter_new_records(&pool, &tender_records)
        .await
        .map_err(|e| Error::from(format!("Failed to filter records: {}", e).as_str()))?;

    let filtered_count = tender_records.len() - new_records.len();
    if filtered_count > 0 {
        info!(
            "Filtered out {} existing records, processing {} new records",
            filtered_count,
            new_records.len()
        );
    }

    // Save new records to database
    let saved_count = if !new_records.is_empty() {
        info!("Saving {} new records to database", new_records.len());
        save_records(&pool, &new_records)
            .await
            .map_err(|e| Error::from(format!("Failed to save records: {}", e).as_str()))?;
        info!("Successfully saved {} records", new_records.len());
        new_records.len()
    } else {
        info!("No new records to save");
        0
    };

    // Send records to appropriate queues
    let queued_count = if !new_records.is_empty() {
        queue_records_for_processing(&new_records)
            .await
            .map_err(|e| Error::from(format!("Failed to queue records: {}", e).as_str()))?
    } else {
        0
    };

    info!("=== POSTGRES DATALOAD COMPLETED ===");

    Ok(Response {
        records_processed: tender_records.len(),
        records_saved: saved_count,
        records_queued: queued_count,
        success: true,
        message: format!(
            "Processed {} records, saved {} new, queued {} for processing",
            tender_records.len(),
            saved_count,
            queued_count
        ),
    })
}

async fn ensure_tables_exist(pool: &Pool<Postgres>) -> Result<(), Error> {
    // Create tender_records table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS tender_records (
            id SERIAL PRIMARY KEY,
            title TEXT NOT NULL,
            resource_id BIGINT NOT NULL UNIQUE,
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
            notification_sent BOOLEAN DEFAULT FALSE,
            notification_sent_at TIMESTAMP WITH TIME ZONE DEFAULT NULL,
            created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Add notification columns if they don't exist (for existing tables)
    sqlx::query(
        r#"
        DO $$
        BEGIN
            IF NOT EXISTS (
                SELECT 1 FROM information_schema.columns
                WHERE table_name='tender_records' AND column_name='notification_sent'
            ) THEN
                ALTER TABLE tender_records ADD COLUMN notification_sent BOOLEAN DEFAULT FALSE;
            END IF;

            IF NOT EXISTS (
                SELECT 1 FROM information_schema.columns
                WHERE table_name='tender_records' AND column_name='notification_sent_at'
            ) THEN
                ALTER TABLE tender_records ADD COLUMN notification_sent_at TIMESTAMP WITH TIME ZONE DEFAULT NULL;
            END IF;
        END $$;
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn filter_new_records(
    pool: &Pool<Postgres>,
    records: &[TenderRecord],
) -> Result<Vec<TenderRecord>, Error> {
    let mut new_records = Vec::new();

    for rec in records {
        // Check if resource_id already exists in database
        let exists: Option<(i64,)> =
            sqlx::query_as("SELECT resource_id FROM tender_records WHERE resource_id = $1")
                .bind(rec.resource_id)
                .fetch_optional(pool)
                .await?;

        if exists.is_none() {
            new_records.push(rec.clone());
        }
    }

    Ok(new_records)
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
                -- Note: We don't update bid column or notification fields to preserve existing data
            "#,
        )
        .bind(&record.title)
        .bind(record.resource_id)
        .bind(&record.contracting_authority)
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

async fn queue_records_for_processing(records: &[TenderRecord]) -> Result<usize, Error> {
    // Initialize AWS SDK
    let aws_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .load()
        .await;
    let sqs_client = SqsClient::new(&aws_config);

    // Split records into PDF and non-PDF
    let (pdf_records, non_pdf_records): (Vec<&TenderRecord>, Vec<&TenderRecord>) =
        records.iter().partition(|r| !r.pdf_url.is_empty());

    let mut queued_count = 0;

    // Send records with PDFs to PDF processing queue
    if !pdf_records.is_empty() {
        let pdf_queue_url = env::var("PDF_PROCESSING_QUEUE_URL")
            .map_err(|_| Error::from("PDF_PROCESSING_QUEUE_URL not set"))?;

        info!(
            "Queuing {} records with PDFs to processing queue",
            pdf_records.len()
        );

        for record in pdf_records {
            let message_body = serde_json::to_string(record)
                .map_err(|e| Error::from(format!("Failed to serialize record: {}", e).as_str()))?;

            match sqs_client
                .send_message()
                .queue_url(&pdf_queue_url)
                .message_body(message_body)
                .send()
                .await
            {
                Ok(_) => {
                    info!("Queued PDF record {} for processing", record.resource_id);
                    queued_count += 1;
                }
                Err(e) => {
                    error!("Failed to queue PDF record {}: {}", record.resource_id, e);
                }
            }
        }
    }

    // Send records without PDFs directly to ML prediction queue
    if !non_pdf_records.is_empty() {
        let ml_queue_url = env::var("ML_PREDICTION_QUEUE_URL")
            .map_err(|_| Error::from("ML_PREDICTION_QUEUE_URL not set"))?;

        info!(
            "Queuing {} records without PDFs to ML prediction queue",
            non_pdf_records.len()
        );

        for record in non_pdf_records {
            let message_body = serde_json::to_string(record)
                .map_err(|e| Error::from(format!("Failed to serialize record: {}", e).as_str()))?;

            match sqs_client
                .send_message()
                .queue_url(&ml_queue_url)
                .message_body(message_body)
                .send()
                .await
            {
                Ok(_) => {
                    info!(
                        "Queued non-PDF record {} for ML prediction",
                        record.resource_id
                    );
                    queued_count += 1;
                }
                Err(e) => {
                    error!(
                        "Failed to queue non-PDF record {}: {}",
                        record.resource_id, e
                    );
                }
            }
        }
    }

    Ok(queued_count)
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
