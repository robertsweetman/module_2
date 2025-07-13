use crate::types::{AISummaryResult, MLPredictionResult, TenderRecord, PdfContent};
use anyhow::Result;
use tracing::{info, debug, warn};
use chrono::Utc;
use serde_json::{json, Value};
use anthropic_sdk;
use std::sync::{Arc, Mutex};

/// AI service for generating summaries using Claude
pub struct AIService {
    api_key: String,
}

impl AIService {
    /// Create new AI service
    pub fn new(api_key: String) -> Self {
        info!("✅ Claude AI service initialized");
        Self { api_key }
    }
    
    /// Generate AI summary - title only version (lightweight)
    pub async fn generate_title_summary(
        &self,
        tender_title: &str,
        contracting_authority: &str,
        ml_prediction: &MLPredictionResult,
        resource_id: i64,
    ) -> Result<AISummaryResult> {
        info!("🤖 Generating title-only AI summary for resource_id: {}", resource_id);
        
        let prompt = format!(
            r#"You are an expert tender analyst. Based on the limited information provided, provide a concise assessment:

TENDER TITLE: "{}"
CONTRACTING AUTHORITY: "{}"
ML PREDICTION: {} (confidence: {:.1}%)
ML REASONING: {}

Please provide:
1. A brief summary of what this tender likely involves
2. Key assessment points based on the title
3. Your recommendation considering the ML prediction
4. Confidence assessment noting the limited information

Format as JSON with fields: summary, key_points (array), recommendation, confidence_assessment"#,
            tender_title,
            contracting_authority,
            if ml_prediction.should_bid { "RECOMMEND BID" } else { "DO NOT BID" },
            ml_prediction.confidence * 100.0,
            ml_prediction.reasoning
        );
        
        let response = self.call_claude(&prompt, 1000).await?;
        self.parse_ai_response(response, "TITLE_ONLY", resource_id)
    }
    
    /// Generate AI summary - full PDF version (comprehensive)
    pub async fn generate_full_summary(
        &self,
        tender: &TenderRecord,
        pdf_content: &PdfContent,
        ml_prediction: &MLPredictionResult,
    ) -> Result<AISummaryResult> {
        info!("🤖 Generating full AI summary for resource_id: {}", tender.resource_id);
        
        // Truncate PDF content if too long (keep within token limits - Claude has higher limits than GPT-4)
        let truncated_pdf = if pdf_content.pdf_text.len() > 15000 {
            warn!("📄 Truncating PDF content from {} to 15000 chars", pdf_content.pdf_text.len());
            format!("{}...[TRUNCATED]", &pdf_content.pdf_text[..15000])
        } else {
            pdf_content.pdf_text.clone()
        };
        
        let detected_codes_str = pdf_content.detected_codes.join(", ");
        
        let prompt = format!(
            r#"You are an expert tender analyst. Analyze this complete tender opportunity:

TENDER DETAILS:
Title: "{}"
Contracting Authority: "{}"
Value: {}
Deadline: {}
Status: "{}"
Procedure: "{}"

PDF CONTENT:
{}

DETECTED PROCUREMENT CODES: {}
CODES COUNT: {}

ML PREDICTION: {} (confidence: {:.1}%)
ML REASONING: {}

Please provide a comprehensive analysis including:
1. Executive summary of the tender opportunity
2. Key requirements and scope
3. Assessment of our suitability based on the content
4. Strategic recommendations
5. Risk factors and considerations
6. Confidence level in your assessment

Format as JSON with fields: summary, key_points (array), recommendation, confidence_assessment"#,
            tender.title,
            tender.contracting_authority,
            tender.value.as_ref().map(|v| v.to_string()).unwrap_or_else(|| "Not specified".to_string()),
            tender.deadline.map(|d| d.to_string()).unwrap_or_else(|| "Not specified".to_string()),
            tender.status,
            tender.procedure,
            truncated_pdf,
            detected_codes_str,
            pdf_content.codes_count,
            if ml_prediction.should_bid { "RECOMMEND BID" } else { "DO NOT BID" },
            ml_prediction.confidence * 100.0,
            ml_prediction.reasoning
        );
        
        let response = self.call_claude(&prompt, 2000).await?;
        self.parse_ai_response(response, "FULL_PDF", tender.resource_id)
    }
    
    /// Call Claude API
    async fn call_claude(&self, prompt: &str, max_tokens: i32) -> Result<String> {
        debug!("🔗 Calling Claude API with prompt length: {}", prompt.len());
        
        let request = anthropic_sdk::Client::new()
            .version("2023-06-01")
            .auth(&self.api_key)
            .model("claude-3-5-sonnet-20241022")
            .messages(&json!([
                {"role": "user", "content": prompt}
            ]))
            .max_tokens(max_tokens)
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build Claude request: {}", e))?;

        let message = Arc::new(Mutex::new(String::new()));
        let message_clone = Arc::clone(&message);

        request
            .execute(move |text| {
                let message_clone = Arc::clone(&message_clone);
                async move {
                    debug!("Claude response chunk: {}", text);
                    let mut message = message_clone.lock().unwrap();
                    *message += &text;
                }
            })
            .await
            .map_err(|e| anyhow::anyhow!("Failed to execute Claude request: {}", e))?;

        let response_text = Arc::try_unwrap(message).unwrap().into_inner().unwrap();
        
        info!("✅ Claude API response received, length: {}", response_text.len());
        Ok(response_text)
    }
    
    /// Parse AI response into structured result
    fn parse_ai_response(&self, response: String, summary_type: &str, resource_id: i64) -> Result<AISummaryResult> {
        debug!("🔍 Parsing Claude response for resource_id: {}", resource_id);
        
        // Try to parse as JSON first
        if let Ok(json_response) = serde_json::from_str::<Value>(&response) {
            let summary = json_response["summary"].as_str().unwrap_or(&response).to_string();
            let key_points = json_response["key_points"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_else(|| vec!["AI response could not be fully parsed".to_string()]);
            let recommendation = json_response["recommendation"].as_str().unwrap_or("See summary").to_string();
            let confidence_assessment = json_response["confidence_assessment"].as_str().unwrap_or("Moderate confidence").to_string();
            
            Ok(AISummaryResult {
                resource_id,
                summary_type: summary_type.to_string(),
                ai_summary: summary,
                key_points,
                recommendation,
                confidence_assessment,
                processing_notes: vec!["Successfully parsed structured Claude response".to_string()],
                created_at: Utc::now(),
            })
        } else {
            // Fallback: use entire response as summary
            warn!("⚠️ Could not parse Claude response as JSON, using as plain text");
            Ok(AISummaryResult {
                resource_id,
                summary_type: summary_type.to_string(),
                ai_summary: response,
                key_points: vec!["Claude response was in plain text format".to_string()],
                recommendation: "Review the summary for recommendations".to_string(),
                confidence_assessment: "Unknown - response format issue".to_string(),
                processing_notes: vec!["Claude response could not be parsed as JSON".to_string()],
                created_at: Utc::now(),
            })
        }
    }
}
