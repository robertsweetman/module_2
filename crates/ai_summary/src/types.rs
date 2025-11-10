use bigdecimal::BigDecimal;
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};

/// Enum to handle different message types that can be sent to AI Summary Lambda
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IncomingMessage {
    AISummary(AISummaryMessage),
    TenderRecord(TenderRecord),
}

/// AI Summary queue message structure (matches ml_bid_predictor)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AISummaryMessage {
    pub resource_id: String,
    pub tender_title: String,
    pub ml_prediction: MLPredictionResult,
    #[serde(default)]
    pub pdf_content: String, // May be truncated/empty - we'll fetch full content if needed
    pub priority: String, // "URGENT" or "NORMAL"
    pub timestamp: DateTime<Utc>,
}

/// ML Prediction result structure (matches ml_bid_predictor)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MLPredictionResult {
    pub should_bid: bool,
    pub confidence: f64,
    #[serde(default = "default_reasoning")]
    pub reasoning: String,
    pub feature_scores: FeatureScores,
}

fn default_reasoning() -> String {
    "No reasoning provided".to_string()
}

/// Feature scores for transparency and debugging (matches ml_bid_predictor)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureScores {
    #[serde(default)]
    pub codes_count_score: f64,
    #[serde(default)]
    pub has_codes_score: f64,
    #[serde(default)]
    pub title_length_score: f64,
    #[serde(default)]
    pub ca_score: f64,
    #[serde(default)]
    pub text_features_score: f64,
    #[serde(default)]
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
    pub ml_processed: Option<bool>,
    pub ml_bid: Option<bool>,
    pub ml_confidence: Option<BigDecimal>,
    pub ml_reasoning: Option<String>,
    pub ml_status: Option<String>,
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

/// SNS message structure for notifications
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SNSMessage {
    pub message_type: String, // "AI_SUMMARY_COMPLETE"
    pub resource_id: String,
    pub title: String,
    pub priority: String, // "HIGH", "URGENT", "LOW"
    pub summary: String,
    pub action_required: String,
    pub timestamp: DateTime<Utc>,
    pub metadata: serde_json::Value,
}

/// Configuration from environment
#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub anthropic_api_key: String,
    pub sns_queue_url: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        // Debug: Check what environment variables are available
        tracing::info!("Loading configuration from environment variables...");

        let database_url = match std::env::var("DATABASE_URL") {
            Ok(url) => {
                tracing::info!("✓ DATABASE_URL found (length: {})", url.len());
                url
            }
            Err(e) => {
                tracing::error!("✗ DATABASE_URL not found: {:?}", e);
                tracing::error!(
                    "Available env vars: {:?}",
                    std::env::vars().map(|(k, _)| k).collect::<Vec<_>>()
                );
                return Err(anyhow::anyhow!("DATABASE_URL environment variable not set"));
            }
        };

        let anthropic_api_key = match std::env::var("ANTHROPIC_API_KEY") {
            Ok(key) => {
                tracing::info!("✓ ANTHROPIC_API_KEY found (length: {})", key.len());
                key
            }
            Err(e) => {
                tracing::error!("✗ ANTHROPIC_API_KEY not found: {:?}", e);
                return Err(anyhow::anyhow!("ANTHROPIC_API_KEY not set"));
            }
        };

        let sns_queue_url = match std::env::var("SNS_QUEUE_URL") {
            Ok(url) => {
                tracing::info!("✓ SNS_QUEUE_URL found (length: {})", url.len());
                url
            }
            Err(e) => {
                tracing::error!("✗ SNS_QUEUE_URL not found: {:?}", e);
                return Err(anyhow::anyhow!("SNS_QUEUE_URL not set"));
            }
        };

        tracing::info!("✅ All configuration loaded successfully");

        Ok(Self {
            database_url,
            anthropic_api_key,
            sns_queue_url,
        })
    }
}
