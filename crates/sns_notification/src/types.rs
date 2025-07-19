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

        Ok(EmailData {
            subject: format!("New High-Priority Tender: {}", msg.title),
            resource_id: msg.resource_id.clone(),
            tender_title: msg.title.clone(),
            contracting_authority: metadata.get("ca")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown Authority")
                .to_string(),
            summary: msg.summary.clone(),
            priority: msg.priority.clone(),
            prediction_confidence: metadata.get("ml_confidence")
                .and_then(|v| v.as_f64())
                .map(|v| v * 100.0), // Convert to percentage
            deadline: metadata.get("deadline")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            estimated_value: metadata.get("value")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            timestamp: msg.timestamp.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
            portal_link: format!("https://etenders.gov.ie/epps/opportunity/opportunityDetailAction.do?opportunityId={}", msg.resource_id),
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
