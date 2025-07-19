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
                .collect()
        };

        let from_email = env::var("FROM_EMAIL")
            .unwrap_or_else(|_| "noreply@etenders.ie".to_string());

        let aws_region = env::var("AWS_REGION")
            .unwrap_or_else(|_| "eu-west-1".to_string());

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
    pub tender_id: Option<String>,
    pub tender_title: Option<String>,
    pub contracting_authority: Option<String>,
    pub priority: String,
    pub summary: Option<String>,
    pub prediction_confidence: Option<f64>,
    pub deadline: Option<DateTime<Utc>>,
    pub estimated_value: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Clone)]
pub struct EmailData {
    pub subject: String,
    pub tender_id: String,
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
    pub fn from_sns_message(sns_message: &SNSMessage) -> Self {
        let tender_id = sns_message.tender_id.clone().unwrap_or_else(|| "Unknown".to_string());
        let tender_title = sns_message.tender_title.clone().unwrap_or_else(|| "No title available".to_string());
        let contracting_authority = sns_message.contracting_authority.clone().unwrap_or_else(|| "Unknown Authority".to_string());
        let summary = sns_message.summary.clone().unwrap_or_else(|| "No summary available".to_string());
        
        let subject = match sns_message.priority.as_str() {
            "URGENT" => format!("🚨 URGENT: New Tender Match - {}", tender_title),
            "HIGH" => format!("⚡ HIGH PRIORITY: Tender Opportunity - {}", tender_title),
            _ => format!("📋 New Tender Notification - {}", tender_title),
        };

        let deadline = sns_message.deadline
            .map(|d| d.format("%Y-%m-%d %H:%M UTC").to_string());

        let portal_link = format!("https://etenders.gov.ie/epps/opportunityDetail.do?opportunityId={}", tender_id);

        EmailData {
            subject,
            tender_id,
            tender_title,
            contracting_authority,
            summary,
            priority: sns_message.priority.clone(),
            prediction_confidence: sns_message.prediction_confidence,
            deadline,
            estimated_value: sns_message.estimated_value.clone(),
            timestamp: sns_message.timestamp.format("%Y-%m-%d %H:%M UTC").to_string(),
            portal_link,
        }
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
