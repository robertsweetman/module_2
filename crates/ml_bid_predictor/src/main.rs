use lambda_runtime::{run, service_fn, tracing, Error, LambdaEvent};
use aws_lambda_events::event::sqs::SqsEvent;
use serde_json::Value;
use tracing::info;

mod ml_predictor;
mod features;
mod queue_handler;
mod types;
mod database;

use database::Database;
use queue_handler::QueueHandler;
use ml_predictor::OptimizedBidPredictor;
use types::TenderRecord;

/// Main lambda handler for ML bid prediction
async fn function_handler(event: LambdaEvent<SqsEvent>) -> Result<Value, Error> {
    let (event, _context) = event.into_parts();
    
    info!("Processing {} SQS records", event.records.len());
    
    // Initialize predictor, queue handler, and database
    let predictor = OptimizedBidPredictor::new();
    let queue_handler = QueueHandler::new().await?;
    let database = Database::new().await?;
    
    let mut processed_count = 0;
    let mut error_count = 0;
    
    for record in &event.records {
        match process_tender_record(&predictor, &queue_handler, &database, record).await {
            Ok(_) => {
                processed_count += 1;
                info!("Successfully processed record {}", processed_count);
            }
            Err(e) => {
                error_count += 1;
                tracing::error!("Error processing record: {}", e);
            }
        }
    }
    
    info!("Batch complete: {} processed, {} errors", processed_count, error_count);
    
    Ok(serde_json::json!({
        "statusCode": 200,
        "body": {
            "processed": processed_count,
            "errors": error_count,
            "message": "ML bid prediction batch completed"
        }
    }))
}

/// Process individual tender record
async fn process_tender_record(
    predictor: &OptimizedBidPredictor,
    queue_handler: &QueueHandler,
    database: &Database,
    record: &impl serde::ser::Serialize,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Parse tender record from SQS message body
    let record_json = serde_json::to_value(record)?;
    let body_str = record_json.get("body")
        .and_then(|v| v.as_str())
        .ok_or("SQS record missing body field")?;
    let tender_record: TenderRecord = serde_json::from_str(body_str)?;
    
    info!("Processing tender: {} (ID: {})", 
          tender_record.title,
          tender_record.resource_id);
    
    // Run ML prediction with optimized threshold (0.050)
    let prediction = predictor.predict(&tender_record)?;
    
    // Handle prediction result based on confidence
    match prediction.should_bid {
        true => {
            info!("🎯 RECOMMENDATION: BID (confidence: {:.3}, threshold: 0.050)", 
                  prediction.confidence);
            
            // Update database with prediction results and set status to 'bid'
            database.update_ml_prediction_results(
                &tender_record.resource_id,
                true,
                prediction.confidence,
                &prediction.reasoning,
                "bid"
            ).await?;
            
            // Send to AI summary queue for further analysis (also sends SNS notification)
            queue_handler.send_to_ai_summary_queue(&tender_record, &prediction).await?;
        }
        false => {
            info!("⏭️  RECOMMENDATION: SKIP (confidence: {:.3}, threshold: 0.050)", 
                  prediction.confidence);
            
            // Update database with prediction results and set status to 'no-bid'
            database.update_ml_prediction_results(
                &tender_record.resource_id,
                false,
                prediction.confidence,
                &prediction.reasoning,
                "no-bid"
            ).await?;
            
            // Just log for monitoring - no queue/notification needed for skips
        }
    }
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize tracing
    tracing::init_default_subscriber();
    
    info!("🚀 Starting ML Bid Predictor Lambda (optimized threshold: 0.050)");
    
    // Run the lambda
    run(service_fn(function_handler)).await
}
