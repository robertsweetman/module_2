use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use anyhow::Result;
use std::env;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub notification_emails: Vec<String>,
    pub from_email: String,
    pub aws_region: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let notification_emails_str = env::var("NOTIFICATION_EMAILS")
            .unwrap_or_else(|_| String::new());
        
        let notification_emails: Vec<String> = if notification_emails_str.is_empty() {
            Vec::new()
        } else {
            notification_emails_str
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .filter(|email| {
                    // Basic email validation - must contain @ and have text before/after it
                    if email.contains('@') && email.split('@').count() == 2 {
                        let parts: Vec<&str> = email.split('@').collect();
                        !parts[0].is_empty() && !parts[1].is_empty() && parts[1].contains('.')
                    } else {
                        eprintln!("WARNING: Invalid email format detected: '{}'", email);
                        false
                    }
                })
                .collect()
        };

        let from_email = env::var("FROM_EMAIL")
            .unwrap_or_else(|_| "etenders-noreply@robertsweetman.com".to_string());

        let aws_region = env::var("AWS_REGION")
            .unwrap_or_else(|_| "eu-west-1".to_string());

        // Log the email configuration for debugging
        eprintln!("Email configuration:");
        eprintln!("  From email: {}", from_email);
        eprintln!("  Notification emails: {:?}", notification_emails);
        eprintln!("  Raw notification emails string: '{}'", notification_emails_str);

        Ok(Config {
            notification_emails,
            from_email,
            aws_region,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SNSMessage {
    pub message_type: String,
    pub resource_id: String,
    pub title: String,
    pub priority: String,
    pub summary: String,
    pub action_required: String,
    pub timestamp: DateTime<Utc>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Serialize, Clone)]
pub struct EmailData {
    pub subject: String,
    pub resource_id: String,
    pub tender_title: String,
    pub contracting_authority: String,
    pub summary: String,
    pub priority: String,
    pub prediction_confidence: Option<f64>,
    pub deadline: Option<String>,
    pub estimated_value: Option<String>,
    pub timestamp: String,
    pub portal_link: String,
    pub ai_summary: String,
    pub key_points: Vec<String>,
    pub recommendation: String,
    pub confidence_assessment: String,
    pub pdf_url: Option<String>,
    pub ml_reasoning: Option<String>,
}

impl EmailData {
    pub fn from_sns_message(msg: &SNSMessage) -> Result<Self, String> {
        // Parse metadata JSON string to serde_json::Value first
        let metadata: serde_json::Value = if let serde_json::Value::String(metadata_str) = &msg.metadata {
            serde_json::from_str(metadata_str)
                .map_err(|e| format!("Failed to parse metadata string: {}", e))?
        } else {
            // If it's already a JSON value, use it directly
            msg.metadata.clone()
        };

        // Debug logging to see what we're working with
        eprintln!("🔍 SNS Message Debug:");
        eprintln!("   Summary: '{}'", msg.summary);
        eprintln!("   Metadata keys: {:?}", metadata.as_object().map(|obj| obj.keys().collect::<Vec<_>>()));
        eprintln!("   AI Summary from metadata: {:?}", metadata.get("ai_summary"));
        eprintln!("   Recommendation from metadata: {:?}", metadata.get("recommendation"));
        eprintln!("   Key points from metadata: {:?}", metadata.get("key_points"));

        Ok(EmailData {
            subject: "Tender Opportunity".to_string(), // Fixed header as requested
            resource_id: msg.resource_id.clone(),
            tender_title: msg.title.clone(),
            contracting_authority: metadata.get("contracting_authority")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown Authority")
                .to_string(),
            summary: msg.summary.clone(), // This should be the simple text summary
            priority: msg.priority.clone(),
            prediction_confidence: metadata.get("ml_prediction")
                .and_then(|ml| ml.get("confidence"))
                .and_then(|v| v.as_f64())
                .map(|v| (v * 100.0).round()), // Convert to percentage and round to nearest whole number
            deadline: metadata.get("deadline")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            estimated_value: metadata.get("estimated_value")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            timestamp: msg.timestamp.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
            portal_link: metadata.get("portal_link")
                .and_then(|v| v.as_str())
                .unwrap_or(&format!("https://etenders.gov.ie/epps/opportunity/opportunityDetailAction.do?opportunityId={}", msg.resource_id))
                .to_string(),
            ai_summary: metadata.get("ai_summary")
                .and_then(|v| v.as_str())
                .unwrap_or(&msg.summary) // Fallback to message summary
                .to_string(),
            key_points: metadata.get("key_points")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter()
                    .filter_map(|item| item.as_str())
                    .map(|s| s.to_string())
                    .collect())
                .unwrap_or_else(|| {
                    eprintln!("⚠️ No key_points found in metadata, using default");
                    vec!["See summary for details".to_string()]
                }),
            recommendation: metadata.get("recommendation")
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| {
                    eprintln!("⚠️ No recommendation found in metadata");
                    "See summary"
                })
                .to_string(),
            confidence_assessment: metadata.get("confidence_assessment")
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| {
                    eprintln!("⚠️ No confidence_assessment found in metadata");
                    "Assessment pending"
                })
                .to_string(),
            pdf_url: metadata.get("pdf_url")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            ml_reasoning: metadata.get("ml_prediction")
                .and_then(|ml| ml.get("reasoning"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        })
    }
}

#[derive(Debug)]
pub enum NotificationPriority {
    Urgent,
    High,
    Normal,
}

impl From<&str> for NotificationPriority {
    fn from(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "URGENT" => NotificationPriority::Urgent,
            "HIGH" => NotificationPriority::High,
            _ => NotificationPriority::Normal,
        }
    }
}
