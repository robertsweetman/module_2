use crate::types::{TenderRecord, MLPredictionResult, AISummaryMessage, SNSMessage, Config};
use aws_sdk_sqs::Client as SqsClient;
use aws_sdk_sns::{Client as SnsClient};
use aws_config::BehaviorVersion;
use anyhow::Result;
use tracing::{info, debug};
use chrono::Utc;
use serde_json;

/// Queue handler for SQS and SNS operations
pub struct QueueHandler {
    sqs_client: SqsClient,
    sns_client: SnsClient,
    config: Config,
}

impl QueueHandler {
    /// Create new queue handler
    pub async fn new() -> Result<Self> {
        let config = Config::from_env()?;
        
        let aws_config = aws_config::defaults(BehaviorVersion::latest())
            .region(aws_config::Region::new(config.aws_region.clone()))
            .load()
            .await;
        
        let sqs_client = SqsClient::new(&aws_config);
        let sns_client = SnsClient::new(&aws_config);
        
        info!("✅ Queue handler initialized");
        Ok(Self {
            sqs_client,
            sns_client,
            config,
        })
    }
    
    /// Send tender result to AI summary queue for LLM processing
    pub async fn send_to_ai_summary_queue(
        &self,
        tender: &TenderRecord,
        prediction: &MLPredictionResult,
    ) -> Result<()> {
        info!("📨 Sending to AI summary queue: {}", tender.resource_id);
        
        let priority = if prediction.should_bid {
            "URGENT"
        } else {
            "NORMAL"
        };
        
        let ai_message = AISummaryMessage {
            resource_id: tender.resource_id.to_string().clone(),
            tender_title: tender.title.clone(),
            ml_prediction: prediction.clone(),
            // Note: PDF content may be truncated/empty here - AI summary lambda will
            // fetch complete content from pdf_content table if needed
            pdf_content: tender.pdf_content.clone().unwrap_or_default(),
            priority: priority.to_string(),
            timestamp: Utc::now(),
        };
        
        let message_body = serde_json::to_string(&ai_message)?;
        
        self.sqs_client
            .send_message()
            .queue_url(&self.config.ai_summary_queue_url)
            .message_body(message_body)
            .send()
            .await?;
        
        info!("✅ Sent to AI summary queue: {}", tender.resource_id);
        
        // NOTE: Removed immediate SNS notification - AI summary service will handle 
        // notifications after applying proper confidence threshold (50%) and Claude analysis
        info!("📋 AI summary service will evaluate confidence threshold and send notification if appropriate");
        
        Ok(())
    }
    
    /// Send SNS notification for predicted bid opportunity
    async fn send_bid_prediction_alert(
        &self,
        tender: &TenderRecord,
        prediction: &MLPredictionResult,
    ) -> Result<()> {
        info!("🎯 Sending bid prediction alert for: {}", tender.resource_id);
        
        let sns_message = SNSMessage {
            message_type: "ML_BID_PREDICTION".to_string(),
            resource_id: tender.resource_id.to_string().clone(),
            title: tender.title.clone(),
            priority: "URGENT".to_string(),
            summary: format!(
                "ML predicts BID opportunity with {:.1}% confidence: {}",
                prediction.confidence * 100.0,
                prediction.reasoning
            ),
            action_required: "Review tender details and initiate bid process if appropriate".to_string(),
            timestamp: Utc::now(),
            metadata: serde_json::json!({
                "ca": tender.contracting_authority,
                "value": tender.value,
                "deadline": tender.deadline,
                "ml_confidence": prediction.confidence,
                "ml_reasoning": prediction.reasoning,
                "feature_scores": prediction.feature_scores
            }),
        };
        
        self.send_sns_notification(&sns_message, "BID_OPPORTUNITY").await?;
        Ok(())
    }
    
    /// Send SNS notification
    async fn send_sns_notification(&self, message: &SNSMessage, subject_prefix: &str) -> Result<()> {
        let subject = format!("[{}] {}", subject_prefix, message.title);
        let message_body = serde_json::to_string_pretty(message)?;
        
        self.sns_client
            .publish()
            .topic_arn(&self.config.sns_topic_arn)
            .subject(subject)
            .message(message_body)
            .send()
            .await?;
        
        debug!("✅ SNS notification sent for: {}", message.resource_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{FeatureScores, TenderRecord};
    use chrono::Utc;

    fn create_test_tender() -> TenderRecord {
        // use chrono::NaiveDate;
        use bigdecimal::BigDecimal;
        use std::str::FromStr;
        
        TenderRecord {
            resource_id: 123,
            title: "Test Software Development".to_string(),
            contracting_authority: "Test Authority".to_string(),
            info: "Test info".to_string(),
            published: None,
            deadline: None,
            procedure: "Open".to_string(),
            status: "Open".to_string(),
            pdf_url: "test.pdf".to_string(),
            awarddate: None,
            value: Some(BigDecimal::from_str("50000").unwrap()),
            cycle: "2024".to_string(),
            bid: None,
            pdf_content: Some("Test PDF content".to_string()),
            detected_codes: Some(vec!["72000000".to_string(), "72200000".to_string()]),
            codes_count: Some(2), // Test with 2 detected codes
            processing_stage: Some("ml_prediction".to_string()),
            ml_bid: None,
            ml_confidence: None,
            ml_reasoning: None,
        }
    }

    fn create_test_prediction() -> MLPredictionResult {
        MLPredictionResult {
            should_bid: true,
            confidence: 0.75,
            reasoning: "HIGH_CONFIDENCE_BID: Has 2 relevant codes, Contains software-related terms".to_string(),
            feature_scores: FeatureScores {
                codes_count_score: 0.35,
                has_codes_score: 0.15,
                title_length_score: 0.05,
                ca_score: 0.08,
                text_features_score: 0.12,
                total_score: 0.75,
            },
        }
    }

    #[test]
    fn test_sns_message_serialization() {
        let tender = create_test_tender();
        
        let sns_message = SNSMessage {
            message_type: "MANUAL_REVIEW".to_string(),
            resource_id: tender.resource_id.to_string().clone(),
            title: tender.title.clone(),
            priority: "HIGH".to_string(),
            summary: "Test summary".to_string(),
            action_required: "Test action".to_string(),
            timestamp: Utc::now(),
            metadata: serde_json::json!({"test": "value"}),
        };
        
        let serialized = serde_json::to_string(&sns_message);
        assert!(serialized.is_ok());
    }

    #[test]
    fn test_ai_summary_message_creation() {
        let tender = create_test_tender();
        let prediction = create_test_prediction();
        
        let ai_message = AISummaryMessage {
            resource_id: tender.resource_id.to_string().clone(),
            tender_title: tender.title.clone(),
            ml_prediction: prediction,
            pdf_content: tender.pdf_content.clone().unwrap_or_default(),
            priority: "URGENT".to_string(),
            timestamp: Utc::now(),
        };
        
        let serialized = serde_json::to_string(&ai_message);
        assert!(serialized.is_ok());
        assert_eq!(ai_message.priority, "URGENT");
    }
}
