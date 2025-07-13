use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc, NaiveDateTime, NaiveDate};
use bigdecimal::BigDecimal;

/// AI Summary queue message structure (matches ml_bid_predictor)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AISummaryMessage {
    pub resource_id: String,
    pub tender_title: String,
    pub ml_prediction: MLPredictionResult,
    pub pdf_content: String, // May be truncated/empty - we'll fetch full content if needed
    pub priority: String,    // "URGENT" or "NORMAL"
    pub timestamp: DateTime<Utc>,
}

/// ML Prediction result structure (matches ml_bid_predictor)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MLPredictionResult {
    pub should_bid: bool,
    pub confidence: f64,
    pub reasoning: String,
    pub feature_scores: FeatureScores,
}

/// Feature scores for transparency and debugging (matches ml_bid_predictor)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureScores {
    pub codes_count_score: f64,
    pub has_codes_score: f64,
    pub title_length_score: f64,
    pub ca_score: f64,
    pub text_features_score: f64,
    pub total_score: f64,
}

/// Complete tender record from database
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
    pub bid: Option<i32>,
    pub pdf_content: Option<String>,
    pub detected_codes: Option<Vec<String>>,
    pub codes_count: Option<i32>,
    pub processing_stage: Option<String>,
}

/// PDF content from the pdf_content table
#[derive(Debug, Clone)]
pub struct PdfContent {
    pub resource_id: i64,
    pub pdf_text: String,
    pub detected_codes: Vec<String>,
    pub codes_count: i32,
    pub extraction_timestamp: DateTime<Utc>,
}

/// AI Summary result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AISummaryResult {
    pub resource_id: i64,
    pub summary_type: String, // "TITLE_ONLY" or "FULL_PDF"
    pub ai_summary: String,
    pub key_points: Vec<String>,
    pub recommendation: String,
    pub confidence_assessment: String,
    pub processing_notes: Vec<String>,
    pub created_at: DateTime<Utc>,
}

/// Configuration from environment
#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub openai_api_key: String,
    pub sns_topic_arn: String,
    pub aws_region: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            database_url: std::env::var("DATABASE_URL")
                .map_err(|_| anyhow::anyhow!("DATABASE_URL not set"))?,
            openai_api_key: std::env::var("OPENAI_API_KEY")
                .map_err(|_| anyhow::anyhow!("OPENAI_API_KEY not set"))?,
            sns_topic_arn: std::env::var("SNS_TOPIC_ARN")
                .map_err(|_| anyhow::anyhow!("SNS_TOPIC_ARN not set"))?,
            aws_region: std::env::var("AWS_REGION").unwrap_or_else(|_| "eu-west-1".to_string()),
        })
    }
}
