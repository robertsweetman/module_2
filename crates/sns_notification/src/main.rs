// crates/sns_notification/src/main.rs
use lambda_runtime::{service_fn, LambdaEvent, Error, run};
use aws_lambda_events::event::sns::SnsEvent;
use anyhow::Result;
use tracing::{info, error};

mod email_service;
mod types;

use email_service::EmailService;
use types::{SNSMessage, Config};

async fn function_handler(event: LambdaEvent<SnsEvent>) -> Result<String, Error> {
    info!("=== SNS NOTIFICATION LAMBDA STARTED ===");
    
    let config = Config::from_env().map_err(|e| {
        error!("Failed to load configuration: {}", e);
        Error::from(e.to_string().as_str())
    })?;
    
    info!("Configuration loaded: {} notification emails configured", config.notification_emails.len());
    
    let email_service = EmailService::new(&config).await
        .map_err(|e| Error::from(format!("Failed to initialize email service: {}", e).as_str()))?;
    
    let mut processed_count = 0;
    
    // Process each SNS record
    for record in event.payload.records {
        let message = record.sns.message;
        info!("Processing SNS message: {}", message);
        
        // Parse the SNS message
        let sns_message: SNSMessage = serde_json::from_str(&message)
            .map_err(|e| {
                error!("Failed to parse SNS message: {}", e);
                Error::from(format!("Failed to parse SNS message: {}", e).as_str())
            })?;
        
        info!("Parsed SNS message - Type: {}, Priority: {}, Tender: {}", 
              sns_message.message_type, 
              sns_message.priority,
              sns_message.tender_id.as_deref().unwrap_or("Unknown"));
        
        // Send email notification
        email_service.send_notification(&sns_message).await
            .map_err(|e| {
                error!("Failed to send email notification: {}", e);
                Error::from(format!("Failed to send email: {}", e).as_str())
            })?;
        
        processed_count += 1;
    }
    
    info!("=== SNS NOTIFICATION LAMBDA COMPLETED ===");
    info!("Successfully processed {} notifications", processed_count);
    Ok(format!("Successfully processed {} notifications", processed_count))
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