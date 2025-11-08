// crates/sns_notification/src/main.rs
use anyhow::Result;
use aws_lambda_events::event::sqs::SqsEvent;
use lambda_runtime::{run, service_fn, Error, LambdaEvent};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::env;
use tracing::{error, info};

mod email_service;
mod types;

use email_service::EmailService;
use types::{Config, SNSMessage};

async fn mark_tender_as_notified(pool: &PgPool, resource_id: i64) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE tender_records
        SET notification_sent = TRUE,
            notification_sent_at = NOW()
        WHERE resource_id = $1
        "#,
    )
    .bind(resource_id)
    .execute(pool)
    .await?;

    info!("Marked tender {} as notified", resource_id);
    Ok(())
}

async fn function_handler(event: LambdaEvent<SqsEvent>) -> Result<String, Error> {
    info!("=== SNS NOTIFICATION LAMBDA STARTED ===");
    info!(
        "Received SQS event with {} records",
        event.payload.records.len()
    );

    let config = Config::from_env().map_err(|e| {
        error!("Failed to load configuration: {}", e);
        Error::from(e.to_string().as_str())
    })?;

    info!(
        "Configuration loaded: {} notification emails configured",
        config.notification_emails.len()
    );

    let email_service = EmailService::new(&config)
        .await
        .map_err(|e| Error::from(format!("Failed to initialize email service: {}", e).as_str()))?;

    // Connect to database to track notifications
    let database_url = env::var("DATABASE_URL")
        .map_err(|_| Error::from("DATABASE_URL environment variable not set"))?;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .map_err(|e| Error::from(format!("Failed to connect to database: {}", e).as_str()))?;
    info!("Connected to database");

    let mut processed_count = 0;

    // Process each SQS record (containing our notification messages)
    for record in event.payload.records {
        if let Some(body) = &record.body {
            info!("Processing SQS message: {}", body);

            // Parse the message directly (our SNSMessage structure)
            let sns_message: SNSMessage = serde_json::from_str(body).map_err(|e| {
                error!("Failed to parse SQS message body: {}", e);
                Error::from(format!("Failed to parse message: {}", e).as_str())
            })?;

            info!(
                "Parsed notification message - Type: {}, Priority: {}, Tender: {}",
                sns_message.message_type, sns_message.priority, sns_message.resource_id
            );

            // Send email notification
            email_service
                .send_notification(&sns_message)
                .await
                .map_err(|e| {
                    error!("Failed to send email notification: {}", e);
                    Error::from(format!("Failed to send email: {}", e).as_str())
                })?;

            // Mark tender as notified in database
            mark_tender_as_notified(&pool, sns_message.resource_id)
                .await
                .map_err(|e| {
                    error!("Failed to mark tender as notified: {}", e);
                    Error::from(format!("Failed to update notification status: {}", e).as_str())
                })?;

            processed_count += 1;
        } else {
            error!("SQS record has no body - skipping");
        }
    }

    info!("=== SNS NOTIFICATION LAMBDA COMPLETED ===");
    info!("Successfully processed {} notifications", processed_count);
    Ok(format!(
        "Successfully processed {} notifications",
        processed_count
    ))
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .without_time()
        .init();

    run(service_fn(function_handler)).await
}
