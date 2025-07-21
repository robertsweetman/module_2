use crate::types::{SNSMessage, Config, AISummaryResult, TenderRecord, MLPredictionResult};
use aws_sdk_sns::Client as SnsClient;
use aws_config::BehaviorVersion;
use anyhow::Result;
use tracing::{info, debug};
use chrono::Utc;
use serde_json;

/// Notification service for sending SNS messages
pub struct NotificationService {
    sns_client: SnsClient,
    topic_arn: String,
}

impl NotificationService {
    /// Create new notification service
    pub async fn new(config: &Config) -> Result<Self> {
        let aws_config = aws_config::defaults(BehaviorVersion::latest())
            .load()
            .await;
        
        let sns_client = SnsClient::new(&aws_config);
        
        info!("✅ Notification service initialized");
        Ok(Self {
            sns_client,
            topic_arn: config.sns_topic_arn.clone(),
        })
    }
    
    /// Determine if notification should be sent based on ML and Claude agreement
    pub fn should_send_notification(
        summary_result: &AISummaryResult,
        ml_prediction: &MLPredictionResult,
    ) -> bool {
        // Enhanced Claude override detection
        let claude_override = summary_result.processing_notes.iter()
            .any(|note| {
                let note_lower = note.to_lowercase();
                note_lower.contains("overrode") || 
                note_lower.contains("override") ||
                note_lower.contains("overriding")
            });
        
        // Enhanced recommendation analysis
        let recommendation_lower = summary_result.recommendation.to_lowercase();
        let ai_summary_lower = summary_result.ai_summary.to_lowercase();
        
        // Multiple ways Claude might say NO BID
        let no_bid_indicators = [
            "no bid", "do not bid", "don't bid", "not bid", "avoid bid",
            "not suitable", "not appropriate", "not relevant", "outside scope",
            "non-it", "not it", "not technical", "unrelated", "irrelevant",
            "construction", "catering", "cleaning", "medical", "school meals",
            "facilities", "maintenance", "security", "transport", "logistics"
        ];
        
        let claude_says_no_bid = no_bid_indicators.iter().any(|&indicator| {
            recommendation_lower.contains(indicator) || ai_summary_lower.contains(indicator)
        }) || claude_override;
        
        // Multiple ways Claude might say YES BID
        let yes_bid_indicators = [
            "bid", "recommend", "pursue", "suitable", "relevant", "appropriate",
            "it consultancy", "software", "technical", "systems"
        ];
        
        let claude_says_bid = yes_bid_indicators.iter().any(|&indicator| {
            recommendation_lower.contains(indicator)
        }) && !claude_says_no_bid;
        
        // Enhanced logging for debugging
        info!("🔍 Notification filtering analysis:");
        info!("   ML prediction: {} (confidence: {:.1}%)", 
              if ml_prediction.should_bid { "BID" } else { "NO-BID" }, 
              ml_prediction.confidence * 100.0);
        info!("   Claude recommendation: '{}'", summary_result.recommendation);
        info!("   Claude override detected: {}", claude_override);
        info!("   Claude says NO BID: {}", claude_says_no_bid);
        info!("   Claude says YES BID: {}", claude_says_bid);
        
        // STRICT FILTERING: Only send notifications when Claude genuinely agrees with bidding
        let should_notify = if ml_prediction.should_bid {
            // ML wants to bid - ONLY notify if Claude explicitly agrees AND doesn't say no bid
            let notify = claude_says_bid && !claude_says_no_bid;
            info!("   ML=BID case: notify={} (claude_says_bid={} && !claude_says_no_bid={})", 
                  notify, claude_says_bid, !claude_says_no_bid);
            notify
        } else {
            // ML doesn't want to bid - very limited notifications for strategic insights only
            info!("   ML=NO-BID case: notify=false (ML doesn't recommend bidding)");
            false
        };
        
        info!("   FINAL DECISION: {}", if should_notify { "SEND NOTIFICATION" } else { "SUPPRESS NOTIFICATION" });
        should_notify
    }
    
    /// Send notification that AI summary is complete
    pub async fn send_summary_complete_notification(
        &self,
        tender: &TenderRecord,
        summary_result: &AISummaryResult,
        ml_prediction: &MLPredictionResult,
    ) -> Result<()> {
        info!("📢 Sending AI summary complete notification for: {}", tender.resource_id);
        
        // Check if Claude overrode the ML prediction
        let claude_override = summary_result.processing_notes.iter()
            .any(|note| note.contains("OVERRODE") || note.contains("overrode"));
        
        let has_non_it_indicators = summary_result.processing_notes.iter()
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
        
        self.send_sns_notification(&sns_message).await?;
        Ok(())
    }
    
    /// Send SNS notification
    async fn send_sns_notification(&self, message: &SNSMessage) -> Result<()> {
        let subject = "Tender Opportunity".to_string();
        
        let message_body = serde_json::to_string_pretty(message)?;
        
        let response = self.sns_client
            .publish()
            .topic_arn(&self.topic_arn)
            .subject(subject)
            .message(message_body)
            .send()
            .await?;
        
        debug!("✅ SNS notification sent for: {} (MessageId: {})", 
               message.resource_id, 
               response.message_id().unwrap_or("unknown"));
        
        info!("📧 Notification sent: {} priority for tender {}", 
              message.priority, message.resource_id);
        
        Ok(())
    }
}
