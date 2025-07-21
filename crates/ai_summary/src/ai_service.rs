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
            r#"You are an expert tender analyst for an IT SERVICE CONSULTANCY specializing in software development, technical support, and IT systems. 

IMPORTANT: You have the authority to OVERRIDE the ML prediction if the tender is clearly outside our IT consultancy scope.

TENDER TITLE: "{}"
CONTRACTING AUTHORITY: "{}"
ML PREDICTION: {} (confidence: {:.1}%)
ML REASONING: {}

OUR COMPANY PROFILE:
- IT Service Consultancy
- Software development and custom applications
- Technical support and IT consulting
- Systems integration and IT infrastructure
- We DO NOT do: construction, catering, cleaning, medical services, physical security, facilities management, mechanical/electrical work

ANALYSIS REQUIRED:
1. Does this tender align with IT consultancy services? (Critical assessment)
2. Brief summary of what this tender involves
3. Your INDEPENDENT recommendation (you may override the ML prediction)
4. Confidence assessment noting the limited information available

If this tender is for non-IT services (construction, catering, cleaning, medical, school meals, etc.), OVERRIDE the ML prediction and your recommendation MUST be "NO BID".

CRITICAL: Your recommendation field must contain either "BID" or "NO BID" - be explicit and clear. If this is not an IT consultancy opportunity, you MUST say "NO BID".

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
            r#"You are an expert tender analyst for an IT SERVICE CONSULTANCY specializing in software development, technical support, and IT systems.

IMPORTANT: You have the authority to OVERRIDE the ML prediction if the tender is clearly outside our IT consultancy scope.

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

OUR COMPANY PROFILE:
- IT Service Consultancy specializing in software development, technical support, IT systems
- Custom software applications and web development  
- IT consulting, systems integration, technical support
- Cloud services, database development, API integrations
- We DO NOT do: construction, building works, catering, cleaning, medical equipment/services, physical security, facilities management, mechanical/electrical installations, architectural services, surveying, waste management

COMPREHENSIVE ANALYSIS REQUIRED:
1. SUITABILITY CHECK: Does this tender genuinely align with IT consultancy services? (Critical assessment)
2. Executive summary of the tender opportunity
3. Key requirements and technical scope analysis
4. INDEPENDENT RECOMMENDATION: You may override the ML prediction if this is clearly non-IT
5. Strategic considerations for our IT consultancy
6. Risk factors and technical considerations
7. Confidence level in your assessment

OVERRIDE GUIDANCE: If this tender is for non-IT services (construction, catering, cleaning, medical equipment, architectural services, school meals, etc.), you should OVERRIDE the ML prediction and your recommendation MUST be "NO BID" regardless of the ML confidence.

CRITICAL: Your recommendation field must contain either "BID" or "NO BID" - be explicit and clear. If this is not an IT consultancy opportunity, you MUST say "NO BID".

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
            
            // Check if Claude overrode the ML prediction
            let mut processing_notes = vec!["Successfully parsed structured Claude response".to_string()];
            
            // Look for override indicators in the response
            let response_lower = response.to_lowercase();
            if response_lower.contains("override") || response_lower.contains("overrid") {
                processing_notes.push("⚠️ Claude OVERRODE the ML prediction".to_string());
                info!("🔄 Claude overrode ML prediction for resource_id: {}", resource_id);
            }
            
            // Check for non-IT keywords in recommendation/summary to flag potential false positives
            let combined_text = format!("{} {}", summary.to_lowercase(), recommendation.to_lowercase());
            let non_it_indicators = [
                "catering", "food service", "cleaning", "maintenance", "construction", 
                "building work", "architectural", "medical", "healthcare", "security guard",
                "waste management", "facilities management", "mechanical", "electrical installation",
                "plumbing", "hvac", "surveying", "legal services", "sewerage", "eeg machine",
                "school meals", "breakfast provision", "lunch provision", "meal service"
            ];
            
            for indicator in &non_it_indicators {
                if combined_text.contains(indicator) {
                    processing_notes.push(format!("🚨 NON-IT INDICATOR DETECTED: {}", indicator));
                    warn!("Non-IT indicator '{}' found in Claude response for resource_id: {}", indicator, resource_id);
                }
            }
            
            // Enhanced NO BID detection in Claude's response
            let no_bid_patterns = [
                "no bid", "do not bid", "don't bid", "not bid", "avoid bid",
                "not suitable", "not appropriate", "not relevant", "outside scope",
                "non-it", "not it related", "not technical", "unrelated", "irrelevant"
            ];
            
            let claude_says_no = no_bid_patterns.iter().any(|&pattern| combined_text.contains(pattern));
            
            if claude_says_no {
                processing_notes.push("🚫 Claude RECOMMENDS NO BID - Non-IT opportunity".to_string());
                info!("🚫 Claude recommends NO BID for resource_id: {} - '{}'", resource_id, recommendation);
            }
            
            Ok(AISummaryResult {
                resource_id,
                summary_type: summary_type.to_string(),
                ai_summary: summary,
                key_points,
                recommendation,
                confidence_assessment,
                processing_notes,
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
