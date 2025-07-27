use crate::types::{SNSMessage, Config, AISummaryResult, TenderRecord, MLPredictionResult};
use aws_sdk_sqs::Client as SqsClient;
use aws_config::BehaviorVersion;
use anyhow::Result;
use tracing::{info, warn};
use chrono::Utc;
use serde_json;

/// Notification service for sending messages to SQS notification queue
pub struct NotificationService {
    sqs_client: SqsClient,
    queue_url: String,
}

impl NotificationService {
    /// Create new notification service
    pub async fn new(config: &Config) -> Result<Self> {
        let aws_config = aws_config::defaults(BehaviorVersion::latest())
            .load()
            .await;
        
        let sqs_client = SqsClient::new(&aws_config);
        
        info!("✅ Notification service initialized for SQS queue");
        Ok(Self {
            sqs_client,
            queue_url: config.sns_queue_url.clone(),
        })
    }
    
    /// Determine if notification should be sent - Claude is now the primary decision maker
    pub fn should_send_notification(
        summary_result: &AISummaryResult,
        _ml_prediction: &MLPredictionResult, // ML is used as fallback when Claude response fails
    ) -> bool {
        info!("🔍 Notification decision analysis (Claude-first approach):");
        
        // PRIMARY DECISION: Claude's recommendation (Claude is the final arbiter)
        let recommendation_lower = summary_result.recommendation.to_lowercase();
        let ai_summary_lower = summary_result.ai_summary.to_lowercase();
        
        // Check if this is a JSON parsing fallback case
        let is_json_fallback = summary_result.recommendation == "Review the summary for recommendations" &&
                              summary_result.processing_notes.iter().any(|note| note.contains("could not be parsed as JSON"));
        
        // Look for explicit BID recommendation from Claude
        let claude_says_bid = recommendation_lower.contains("bid") && !recommendation_lower.contains("no bid");
        
        info!("   Claude recommendation: '{}'", summary_result.recommendation);
        info!("   Is JSON parsing fallback: {}", is_json_fallback);
        info!("   Claude says BID: {}", claude_says_bid);
        
        // Special handling for JSON parsing fallback - analyze the summary content
        if is_json_fallback {
            info!("🔍 JSON parsing fallback detected - analyzing summary content for BID indicators");
            
            // Look for positive IT indicators in the summary
            let it_keywords = [
                "software development", "it consultancy", "it consulting", "technical support",
                "system implementation", "database", "web development", "api", "cloud",
                "cybersecurity", "network", "server", "application", "programming",
                "crm system", "software", "digital platform", "technical", "it service"
            ];
            
            let has_it_keywords = it_keywords.iter().any(|&keyword| ai_summary_lower.contains(keyword));
            
            // Look for explicit positive language
            let positive_indicators = [
                "legitimate it", "clear it", "genuine it", "solid it opportunity",
                "this is an it", "it consultancy opportunity", "technical opportunity"
            ];
            
            let has_positive_indicators = positive_indicators.iter().any(|&indicator| ai_summary_lower.contains(indicator));
            
            info!("   Has IT keywords: {}", has_it_keywords);
            info!("   Has positive indicators: {}", has_positive_indicators);
            
            // If summary indicates clear IT opportunity, treat as BID recommendation
            if has_it_keywords && has_positive_indicators {
                info!("   ✅ FALLBACK APPROVAL: Summary indicates clear IT opportunity despite JSON parsing issue");
                // Continue with other checks below
            } else {
                info!("   ❌ SUPPRESSED: Fallback analysis does not indicate clear IT opportunity");
                return false;
            }
        } else if !claude_says_bid {
            info!("   ❌ SUPPRESSED: Claude does not recommend BID");
            return false;
        }
        
        // ENHANCED NON-IT DETECTION: Look for non-IT indicators in Claude's analysis
        let combined_text = format!("{} {} {}", 
                                   ai_summary_lower,
                                   recommendation_lower,
                                   summary_result.key_points.join(" ").to_lowercase());
        
        // Comprehensive non-IT keywords (strict filtering)
        let non_it_keywords = [
            // Construction & Building
            "construction", "building work", "renovation", "refurbishment", "extension", 
            "architectural", "structural", "civil engineering", "building maintenance",
            
            // Catering & Food Services  
            "catering", "food service", "school meals", "breakfast", "lunch", "dinner",
            "meal provision", "kitchen", "dining", "food preparation", "canteen",
            
            // Cleaning & Maintenance
            "cleaning", "cleaning service", "janitorial", "housekeeping", "grounds maintenance",
            "facilities management", "waste management", "refuse collection",
            
            // Medical & Healthcare
            "medical", "healthcare", "clinical", "hospital", "patient", "medical equipment",
            "eeg machine", "medical device", "health service", "medical supply",
            
            // Physical Security & Safety
            "security guard", "security service", "cctv installation", "access control installation",
            "physical security", "patrol", "security personnel", "safety equipment",
            
            // Utilities & Infrastructure  
            "plumbing", "electrical installation", "heating", "ventilation", "hvac",
            "water supply", "sewerage", "drainage", "utilities", "gas installation",
            
            // Professional Services (Non-IT)
            "legal service", "accounting", "surveying", "architectural service", 
            "hr service", "recruitment", "legal advice", "financial advice",
            
            // Transport & Logistics
            "transport", "delivery", "logistics", "fleet management", "vehicle maintenance"
        ];
        
        let detected_non_it: Vec<&str> = non_it_keywords.iter()
            .filter(|&&keyword| combined_text.contains(keyword))
            .copied()
            .collect();
        
        if !detected_non_it.is_empty() {
            warn!("🚨 NON-IT INDICATORS DETECTED: {:?}", detected_non_it);
            info!("   ❌ SUPPRESSED: Non-IT keywords found in Claude analysis");
            return false;
        }
        
        // Enhanced NO BID pattern detection
        let no_bid_patterns = [
            "no bid", "do not bid", "don't bid", "not suitable", "not appropriate",
            "not relevant", "outside scope", "non-it", "not it related", "not technical",
            "unrelated", "irrelevant", "avoid", "reject", "not recommended"
        ];
        
        let claude_says_no = no_bid_patterns.iter().any(|&pattern| combined_text.contains(pattern));
        
        if claude_says_no {
            info!("   ❌ SUPPRESSED: Claude indicates NO BID in analysis");
            return false;
        }
        
        // FINAL DECISION: Send notification only if Claude clearly recommends BID with no red flags
        info!("   ✅ APPROVED: Claude recommends BID with no red flags detected");
        true
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
        
        self.send_sqs_notification(&sns_message).await?;
        Ok(())
    }
    
    /// Send notification message to SQS queue
    async fn send_sqs_notification(&self, message: &SNSMessage) -> Result<()> {
        let message_body = serde_json::to_string(message)?;
        
        info!("📤 Sending notification to SQS queue: {}", self.queue_url);
        
        let response = self.sqs_client
            .send_message()
            .queue_url(&self.queue_url)
            .message_body(message_body)
            .send()
            .await?;
        
        info!("✅ SQS notification sent for tender {} (MessageId: {})", 
               message.resource_id, 
               response.message_id().unwrap_or("unknown"));
        
        Ok(())
    }
}
