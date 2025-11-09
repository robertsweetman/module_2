use crate::types::{AISummaryResult, Config, MLPredictionResult, SNSMessage, TenderRecord};
use anyhow::Result;
use aws_config::BehaviorVersion;
use aws_sdk_sqs::Client as SqsClient;
use chrono::Utc;
use serde_json;
use tracing::{info, warn};

/// Notification service for sending messages to SQS notification queue
pub struct NotificationService {
    sqs_client: SqsClient,
    queue_url: String,
}

impl NotificationService {
    /// Create new notification service
    pub async fn new(config: &Config) -> Result<Self> {
        let aws_config = aws_config::defaults(BehaviorVersion::latest()).load().await;

        let sqs_client = SqsClient::new(&aws_config);

        info!("✅ Notification service initialized for SQS queue");
        Ok(Self {
            sqs_client,
            queue_url: config.sns_queue_url.clone(),
        })
    }

    /// Determine if notification should be sent - Claude is the expert, trust its decision
    pub fn should_send_notification(
        summary_result: &AISummaryResult,
        ml_prediction: &MLPredictionResult,
    ) -> bool {
        info!("🔍 Notification decision analysis (Claude-first approach):");

        // PRIMARY DECISION: Claude's recommendation (Claude is the final arbiter)
        let recommendation_lower = summary_result.recommendation.to_lowercase();

        // Check if this is a JSON parsing fallback case
        let is_json_fallback = summary_result.recommendation
            == "Review the summary for recommendations"
            && summary_result
                .processing_notes
                .iter()
                .any(|note| note.contains("could not be parsed as JSON"));

        info!(
            "   Claude recommendation: '{}'",
            summary_result.recommendation
        );
        info!("   Is JSON parsing fallback: {}", is_json_fallback);

        // Special handling for JSON parsing fallback - fall back to ML prediction
        if is_json_fallback {
            info!("🔍 JSON parsing fallback detected - using ML prediction as backup");
            info!(
                "   ML prediction: {} (confidence: {:.1}%)",
                if ml_prediction.should_bid {
                    "BID"
                } else {
                    "NO BID"
                },
                ml_prediction.confidence * 100.0
            );

            if ml_prediction.should_bid {
                info!("   ✅ FALLBACK APPROVAL: ML recommends BID, JSON parsing failed");
                return true;
            } else {
                info!("   ❌ SUPPRESSED: ML recommends NO BID, JSON parsing failed");
                return false;
            }
        }

        // Look for explicit BID recommendation from Claude
        let claude_says_bid =
            recommendation_lower.contains("bid") && !recommendation_lower.contains("no bid");

        info!("   Claude says BID: {}", claude_says_bid);

        if claude_says_bid {
            info!("   ✅ APPROVED: Claude recommends BID - trusting AI expert decision");
            true
        } else {
            info!("   ❌ SUPPRESSED: Claude does not recommend BID");
            false
        }
    }

    /// Send notification that AI summary is complete
    pub async fn send_summary_complete_notification(
        &self,
        tender: &TenderRecord,
        summary_result: &AISummaryResult,
        ml_prediction: &MLPredictionResult,
    ) -> Result<()> {
        info!(
            "📢 Sending AI summary complete notification for: {}",
            tender.resource_id
        );

        // Check if Claude overrode the ML prediction
        let claude_override = summary_result
            .processing_notes
            .iter()
            .any(|note| note.contains("OVERRODE") || note.contains("overrode"));

        let has_non_it_indicators = summary_result
            .processing_notes
            .iter()
            .any(|note| note.contains("NON-IT INDICATOR"));

        let priority = if claude_override && ml_prediction.should_bid {
            // This case should rarely happen now due to notification filtering
            "CRITICAL" // Claude overrode ML's bid recommendation - needs immediate attention
        } else if ml_prediction.should_bid && !has_non_it_indicators {
            "URGENT" // ML bid recommendation confirmed by Claude
        } else if has_non_it_indicators {
            "MEDIUM" // Has some concerns but not filtered out
        } else if summary_result.summary_type == "FULL_PDF" {
            "HIGH"
        } else {
            "NORMAL"
        };

        let action_required = if claude_override && ml_prediction.should_bid {
            "🚨 CRITICAL: Claude AI OVERRODE ML bid recommendation - review immediately for accuracy"
        } else if ml_prediction.should_bid {
            "REVIEW IMMEDIATELY: ML recommends bidding - Claude analysis confirms opportunity"
        } else if has_non_it_indicators {
            "⚠️ Review recommended: Some non-IT indicators detected but passed initial screening"
        } else {
            "Review completed AI summary for strategic assessment"
        };

        let sns_message = SNSMessage {
            message_type: "AI_SUMMARY_COMPLETE".to_string(),
            resource_id: tender.resource_id.to_string(),
            title: tender.title.clone(),
            priority: priority.to_string(),
            summary: summary_result.ai_summary.clone(), // Simple text summary for email service to format
            action_required: action_required.to_string(),
            timestamp: Utc::now(),
            metadata: serde_json::json!({
                "resource_id": tender.resource_id,
                "contracting_authority": tender.contracting_authority,
                "estimated_value": tender.value,
                "deadline": tender.deadline,
                "summary_type": summary_result.summary_type,
                "claude_override": claude_override,
                "has_non_it_indicators": has_non_it_indicators,
                "processing_notes": summary_result.processing_notes,
                "notification_sent": true,
                "ml_prediction": {
                    "should_bid": ml_prediction.should_bid,
                    "confidence": ml_prediction.confidence,
                    "reasoning": ml_prediction.reasoning
                },
                "ml_status": tender.ml_status,
                "ml_processed": tender.ml_processed,
                "ai_summary": summary_result.ai_summary,
                "key_points": summary_result.key_points,
                "recommendation": summary_result.recommendation,
                "confidence_assessment": summary_result.confidence_assessment,
                "pdf_url": tender.pdf_url,
                "status": tender.status,
                "procedure": tender.procedure,
                "portal_link": format!("https://etenders.gov.ie/epps/opportunity/opportunityDetailAction.do?opportunityId={}", tender.resource_id)
            }),
        };

        self.send_sqs_notification(&sns_message).await?;
        Ok(())
    }

    /// Send notification message to SQS queue
    async fn send_sqs_notification(&self, message: &SNSMessage) -> Result<()> {
        let message_body = serde_json::to_string(message)?;

        info!("📤 Sending notification to SQS queue: {}", self.queue_url);

        let response = self
            .sqs_client
            .send_message()
            .queue_url(&self.queue_url)
            .message_body(message_body)
            .send()
            .await?;

        info!(
            "✅ SQS notification sent for tender {} (MessageId: {})",
            message.resource_id,
            response.message_id().unwrap_or("unknown")
        );

        Ok(())
    }
}
