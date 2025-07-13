use crate::types::{AISummaryResult, MLPredictionResult, TenderRecord, PdfContent};
use anyhow::Result;
use tracing::{info, debug, warn};
use chrono::Utc;
use reqwest::Client;
use serde_json::{json, Value};

/// AI service for generating summaries using OpenAI
pub struct AIService {
    client: Client,
    api_key: String,
}

impl AIService {
    /// Create new AI service
    pub fn new(api_key: String) -> Self {
        let client = Client::new();
        info!("✅ AI service initialized");
        Self { client, api_key }
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
        
        let response = self.call_openai(&prompt, 800).await?;
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
        
        // Truncate PDF content if too long (keep within token limits)
        let truncated_pdf = if pdf_content.pdf_text.len() > 8000 {
            warn!("📄 Truncating PDF content from {} to 8000 chars", pdf_content.pdf_text.len());
            format!("{}...[TRUNCATED]", &pdf_content.pdf_text[..8000])
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
        
        let response = self.call_openai(&prompt, 1500).await?;
        self.parse_ai_response(response, "FULL_PDF", tender.resource_id)
    }
    
    /// Call OpenAI API
    async fn call_openai(&self, prompt: &str, max_tokens: u32) -> Result<String> {
        debug!("🔗 Calling OpenAI API with prompt length: {}", prompt.len());
        
        let request_body = json!({
            "model": "gpt-4",
            "messages": [
                {
                    "role": "system",
                    "content": "You are an expert tender analyst with deep knowledge of public procurement processes, technical requirements assessment, and business strategy."
                },
                {
                    "role": "user", 
                    "content": prompt
                }
            ],
            "max_tokens": max_tokens,
            "temperature": 0.3
        });
        
        let response = self.client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;
        
        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("OpenAI API error: {}", error_text));
        }
        
        let response_json: Value = response.json().await?;
        let content = response_json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("No content in OpenAI response"))?;
        
        info!("✅ OpenAI API response received, length: {}", content.len());
        Ok(content.to_string())
    }
    
    /// Parse AI response into structured result
    fn parse_ai_response(&self, response: String, summary_type: &str, resource_id: i64) -> Result<AISummaryResult> {
        debug!("🔍 Parsing AI response for resource_id: {}", resource_id);
        
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
                processing_notes: vec!["Successfully parsed structured AI response".to_string()],
                created_at: Utc::now(),
            })
        } else {
            // Fallback: use entire response as summary
            warn!("⚠️ Could not parse AI response as JSON, using as plain text");
            Ok(AISummaryResult {
                resource_id,
                summary_type: summary_type.to_string(),
                ai_summary: response,
                key_points: vec!["AI response was in plain text format".to_string()],
                recommendation: "Review the summary for recommendations".to_string(),
                confidence_assessment: "Unknown - response format issue".to_string(),
                processing_notes: vec!["AI response could not be parsed as JSON".to_string()],
                created_at: Utc::now(),
            })
        }
    }
}
