use lambda_runtime::{service_fn, LambdaEvent, Error, run};
use aws_lambda_events::event::sqs::SqsEvent;
use tracing::{info, error, warn};
use tracing_subscriber;
use serde_json;
use anyhow::Result;

mod types;
mod database;
mod ai_service;
mod notification_service;

use types::{AISummaryMessage, Config};
use database::Database;
use ai_service::AIService;
use notification_service::NotificationService;

async fn function_handler(event: LambdaEvent<SqsEvent>) -> Result<String, Error> {
    info!("=== AI SUMMARY LAMBDA STARTED ===");
    
    // Initialize configuration
    let config = Config::from_env().map_err(|e| {
        error!("Failed to load configuration: {}", e);
        Error::from(e.to_string().as_str())
    })?;
    
    // Initialize services
    let database = Database::new(&config).await.map_err(|e| {
        error!("Failed to initialize database: {}", e);
        Error::from(e.to_string().as_str())
    })?;
    
    let ai_service = AIService::new(config.anthropic_api_key.clone());
    
    let notification_service = NotificationService::new(&config).await.map_err(|e| {
        error!("Failed to initialize notification service: {}", e);
        Error::from(e.to_string().as_str())
    })?;
    
    // Process SQS records
    let sqs_records = &event.payload.records;
    info!("Processing {} SQS records", sqs_records.len());
    
    for record in sqs_records {
        if let Some(body) = &record.body {
            match process_summary_message(body, &database, &ai_service, &notification_service).await {
                Ok(_) => info!("✅ Successfully processed message"),
                Err(e) => {
                    error!("❌ Failed to process message: {}", e);
                    // Continue processing other messages rather than failing entire batch
                }
            }
        } else {
            warn!("⚠️ SQS record has no body, skipping");
        }
    }
    
    Ok("Completed AI summary processing".to_string())
}

async fn process_summary_message(
    message_body: &str,
    database: &Database,
    ai_service: &AIService,
    notification_service: &NotificationService,
) -> Result<()> {
    info!("🔄 Processing AI summary message");
    
    // Parse the incoming message
    let ai_message: AISummaryMessage = serde_json::from_str(message_body)?;
    let resource_id: i64 = ai_message.resource_id.parse()?;
    
    info!("📋 Processing summary for resource_id: {}, priority: {}", 
          resource_id, ai_message.priority);
    
    // Get tender record for context (needed for both processing paths and notification)
    let tender = database.get_tender_record(resource_id).await?
        .ok_or_else(|| anyhow::anyhow!("Tender record not found for resource_id: {}", resource_id))?;
    
    // Determine processing strategy based on available content
    let summary_result = if ai_message.pdf_content.is_empty() || ai_message.pdf_content.len() < 100 {
        info!("📝 Using title-only processing (no/minimal PDF content)");
        
        ai_service.generate_title_summary(
            &tender.title,
            &tender.contracting_authority,
            &ai_message.ml_prediction,
            resource_id,
        ).await?
    } else {
        info!("📄 Checking if we need to fetch complete PDF content");
        
        // Check if we have full PDF content or need to fetch from database
        let pdf_content = if ai_message.pdf_content.len() > 1000 {
            info!("✅ Using PDF content from message (length: {})", ai_message.pdf_content.len());
            
            // Create PdfContent from message data
            crate::types::PdfContent {
                resource_id,
                pdf_text: ai_message.pdf_content,
                detected_codes: vec![], // Will be populated from database if available
                codes_count: 0,
                extraction_timestamp: chrono::Utc::now(),
            }
        } else {
            info!("🔍 Fetching complete PDF content from database");
            
            database.get_pdf_content(resource_id).await?
                .ok_or_else(|| anyhow::anyhow!("No PDF content found in database for resource_id: {}", resource_id))?
        };
        
        info!("📊 Using full PDF processing (PDF text length: {})", pdf_content.pdf_text.len());
        ai_service.generate_full_summary(&tender, &pdf_content, &ai_message.ml_prediction).await?
    };
    
    // Store the result
    database.store_ai_summary(&summary_result).await?;
    
    info!("✅ AI summary completed for resource_id: {} (type: {})", 
          resource_id, summary_result.summary_type);
    
    // Determine if we should send notification based on ML and Claude agreement
    if NotificationService::should_send_notification(&summary_result, &ai_message.ml_prediction) {
        info!("📧 Sending notification - Claude and ML are aligned or Claude confirms bid opportunity");
        
        // Send notification about completed AI summary
        notification_service.send_summary_complete_notification(
            &tender,
            &summary_result,
            &ai_message.ml_prediction,
        ).await?;
        
        // Log summary for monitoring
        info!("📋 Summary preview (email sent): {}", 
              if summary_result.ai_summary.len() > 200 {
                  format!("{}...", &summary_result.ai_summary[..200])
              } else {
                  summary_result.ai_summary.clone()
              });
    } else {
        info!("🚫 Suppressing notification - Claude overrode ML recommendation or identified non-IT tender");
        info!("📋 Summary preview (no email sent): {}", 
              if summary_result.ai_summary.len() > 200 {
                  format!("{}...", &summary_result.ai_summary[..200])
              } else {
                  summary_result.ai_summary.clone()
              });
    }
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .without_time()
        .init();
    
    info!("=== AI Summary Lambda Starting ===");
    
    // Run the lambda
    run(service_fn(function_handler)).await
}
