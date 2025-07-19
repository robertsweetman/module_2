use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc, NaiveDateTime, NaiveDate};
use bigdecimal::BigDecimal;

/// Tender record structure matching the database schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenderRecord {
    pub resource_id: i64,
    pub title: String,
    pub contracting_authority: String,
    pub info: String,
    pub published: Option<NaiveDateTime>,
    pub deadline: Option<NaiveDateTime>,
    pub procedure: String,
    pub status: String,
    pub pdf_url: String,
    pub awarddate: Option<NaiveDate>,
    pub value: Option<BigDecimal>,
    pub cycle: String,
    pub bid: Option<i32>, // 1 = bid, 0 = no bid, NULL = unlabeled
    pub pdf_content: Option<String>, // Added by pdf_processing
    pub detected_codes: Option<Vec<String>>, // Added by pdf_processing - actual codes found
    pub codes_count: Option<i32>, // Added by pdf_processing - count of detected codes
    pub processing_stage: Option<String>, // Track pipeline stage
    
    // ML prediction results (added by ml_bid_predictor)
    pub ml_bid: Option<bool>,          // ML prediction result
    pub ml_confidence: Option<f64>,    // ML confidence score
    pub ml_reasoning: Option<String>,  // ML reasoning/category
}

/// ML Prediction result structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MLPredictionResult {
    pub should_bid: bool,
    pub confidence: f64,
    pub reasoning: String,
    pub feature_scores: FeatureScores,
}

/// Feature scores for transparency and debugging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureScores {
    pub codes_count_score: f64,
    pub has_codes_score: f64,
    pub title_length_score: f64,
    pub ca_score: f64,
    pub text_features_score: f64,
    pub total_score: f64,
}

/// Queue message structure for SQS
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueMessage {
    pub resource_id: String,
    pub message_type: String,
    pub timestamp: DateTime<Utc>,
    pub metadata: Option<serde_json::Value>,
}

/// AI Summary queue message structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AISummaryMessage {
    pub resource_id: String,
    pub tender_title: String,
    pub ml_prediction: MLPredictionResult,
    pub pdf_content: String,
    pub priority: String,           // "URGENT" or "NORMAL"
    pub timestamp: DateTime<Utc>,
}

/// SNS message structure for notifications
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SNSMessage {
    pub message_type: String,       // "MANUAL_REVIEW" or "ML_RESULT"
    pub resource_id: String,
    pub title: String,
    pub priority: String,           // "HIGH", "URGENT", "LOW"
    pub summary: String,
    pub action_required: String,
    pub timestamp: DateTime<Utc>,
    pub metadata: serde_json::Value,
}

/// Feature vector for ML processing
#[derive(Debug, Clone)]
pub struct FeatureVector {
    pub codes_count: f64,
    pub has_codes: f64,
    pub title_length: f64,
    pub ca_encoded: f64,
    pub exclusion_score: f64,  // NEW: Non-IT sector filtering
    pub tfidf_software: f64,
    pub tfidf_support: f64,
    pub tfidf_provision: f64,
    pub tfidf_computer: f64,
    pub tfidf_services: f64,
    pub tfidf_systems: f64,
    pub tfidf_management: f64,
    pub tfidf_works: f64,
    pub tfidf_package: f64,
    pub tfidf_technical: f64,
}

impl FeatureVector {
    pub fn to_array(&self) -> [f64; 15] {
        [
            self.codes_count,
            self.has_codes,
            self.title_length,
            self.ca_encoded,
            self.exclusion_score,
            self.tfidf_software,
            self.tfidf_support,
            self.tfidf_provision,
            self.tfidf_computer,
            self.tfidf_services,
            self.tfidf_systems,
            self.tfidf_management,
            self.tfidf_works,
            self.tfidf_package,
            self.tfidf_technical,
        ]
    }
}

/// Environment configuration
#[derive(Debug, Clone)]
pub struct Config {
    pub ai_summary_queue_url: String,
    pub sns_topic_arn: String,
    pub aws_region: String,
}

impl Config {
    pub fn from_env() -> Result<Self, std::env::VarError> {
        Ok(Self {
            ai_summary_queue_url: std::env::var("AI_SUMMARY_QUEUE_URL")?,
            sns_topic_arn: std::env::var("SNS_TOPIC_ARN")?,
            aws_region: std::env::var("AWS_REGION").unwrap_or_else(|_| "eu-west-1".to_string()),
        })
    }
}
