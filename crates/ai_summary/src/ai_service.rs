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
    
    /// Safely truncate a string at the specified byte position, respecting UTF-8 character boundaries
    fn safe_truncate(text: &str, max_bytes: usize) -> String {
        if text.len() <= max_bytes {
            return text.to_string();
        }
        
        let mut end = max_bytes;
        while !text.is_char_boundary(end) && end > 0 {
            end -= 1;
        }
        format!("{}...", &text[..end])
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

🚨 CRITICAL: You are the FINAL DECISION MAKER. The ML prediction is just a rough filter - you have full authority to override it.

🚨 DEFAULT TO "NO BID" unless this is CLEARLY an IT consultancy opportunity. We get too many false positives.

TENDER TITLE: "{}"
CONTRACTING AUTHORITY: "{}"
ML PREDICTION: {} (confidence: {:.1}% - treat as unreliable)
ML REASONING: {}

🎯 OUR STRICT IT CONSULTANCY SCOPE:
✅ SOFTWARE DEVELOPMENT: Custom applications, web development, mobile apps
✅ IT CONSULTING: Systems analysis, technical architecture, IT strategy
✅ TECHNICAL SUPPORT: IT helpdesk, system administration, technical maintenance
✅ SYSTEMS INTEGRATION: API development, database design, cloud services
✅ IT INFRASTRUCTURE: Network setup, server configuration, cybersecurity

🚫 WE ABSOLUTELY DO NOT DO:
❌ CONSTRUCTION & BUILDING: Any physical building work, renovations, extensions
❌ CATERING & FOOD: School meals, catering services, food provision, kitchen equipment
❌ CLEANING & MAINTENANCE: Cleaning services, grounds maintenance, facilities management  
❌ MEDICAL & HEALTHCARE: Medical equipment, healthcare services, clinical supplies
❌ PHYSICAL SECURITY: Security guards, CCTV installation, access control systems
❌ UTILITIES & INFRASTRUCTURE: Water, sewerage, electrical installation, plumbing, HVAC
❌ PROFESSIONAL SERVICES: Legal, accounting, architectural, surveying, consulting (non-IT)
❌ SUPPLIES & EQUIPMENT: Office supplies, furniture, vehicles, non-IT equipment

🔍 ANALYSIS REQUIRED:
1. 🚨 IMMEDIATE REJECTION CHECK: Is this obviously non-IT? (construction, catering, cleaning, medical, etc.)
2. IT SCOPE VERIFICATION: Does this genuinely require IT consultancy expertise?
3. RISK ASSESSMENT: Could this be a false positive from keyword matching?
4. FINAL RECOMMENDATION: BID only if this is clearly within our IT consultancy scope

⚠️ OVERRIDE GUIDANCE: 
- If you see ANY non-IT keywords (construction, catering, cleaning, medical, security guards, etc.), OVERRIDE to "NO BID"
- If the tender scope is unclear or ambiguous, OVERRIDE to "NO BID" 
- Only recommend "BID" if you are confident this is genuine IT consultancy work

🎯 RESPONSE FORMAT: Your recommendation field MUST contain either "BID" or "NO BID" - be explicit and conservative.

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
            format!("{}[TRUNCATED]", Self::safe_truncate(&pdf_content.pdf_text, 15000))
        } else {
            pdf_content.pdf_text.clone()
        };
        
        let detected_codes_str = pdf_content.detected_codes.join(", ");
        
        let prompt = format!(
            r#"You are an expert tender analyst for an IT SERVICE CONSULTANCY specializing in software development, technical support, and IT systems.

🚨 CRITICAL: You are the FINAL DECISION MAKER. The ML prediction is just a rough filter - you have full authority to override it.

🚨 DEFAULT TO "NO BID" unless this is CLEARLY an IT consultancy opportunity. We get too many false positives.

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

ML PREDICTION: {} (confidence: {:.1}% - treat as unreliable)
ML REASONING: {}

🎯 OUR STRICT IT CONSULTANCY SCOPE:
✅ SOFTWARE DEVELOPMENT: Custom applications, web development, mobile apps, databases
✅ IT CONSULTING: Systems analysis, technical architecture, IT strategy, digital transformation
✅ TECHNICAL SUPPORT: IT helpdesk, system administration, technical maintenance, user training
✅ SYSTEMS INTEGRATION: API development, database design, cloud services, software integration
✅ IT INFRASTRUCTURE: Network setup, server configuration, cybersecurity, IT procurement

🚫 WE ABSOLUTELY DO NOT DO:
❌ CONSTRUCTION & BUILDING: Any physical building work, renovations, extensions, refurbishments
❌ CATERING & FOOD: School meals, catering services, food provision, kitchen equipment, dining services
❌ CLEANING & MAINTENANCE: Cleaning services, grounds maintenance, facilities management, janitorial
❌ MEDICAL & HEALTHCARE: Medical equipment, healthcare services, clinical supplies, patient care
❌ PHYSICAL SECURITY: Security guards, CCTV installation, access control systems, patrol services
❌ UTILITIES & INFRASTRUCTURE: Water, sewerage, electrical installation, plumbing, HVAC, heating
❌ PROFESSIONAL SERVICES: Legal, accounting, architectural, surveying, HR, non-IT consulting
❌ SUPPLIES & EQUIPMENT: Office supplies, furniture, vehicles, non-IT equipment, stationery
❌ TRANSPORT & LOGISTICS: Vehicle services, delivery, transport, fleet management
❌ WASTE MANAGEMENT: Waste collection, recycling, environmental services

🔍 COMPREHENSIVE ANALYSIS:
1. 🚨 IMMEDIATE REJECTION CHECK: Scan for obvious non-IT indicators in title and content
2. CONTENT DEEP DIVE: Analyze the full PDF content for hidden non-IT requirements
3. PROCUREMENT CODES: Evaluate if codes indicate non-IT procurement categories
4. SCOPE VERIFICATION: Does this genuinely require IT consultancy expertise?
5. FALSE POSITIVE ASSESSMENT: Could this be a keyword false positive?
6. FINAL EXPERT JUDGMENT: Apply human-level reasoning to the decision

⚠️ OVERRIDE GUIDANCE - BE EXTREMELY CONSERVATIVE:
- If you see ANY non-IT keywords in title or content, OVERRIDE to "NO BID"
- If procurement codes suggest non-IT categories, OVERRIDE to "NO BID"
- If the tender scope includes ANY physical work/services, OVERRIDE to "NO BID"
- If requirements are unclear or ambiguous, OVERRIDE to "NO BID"
- Only recommend "BID" if you are highly confident this is pure IT consultancy work

🎯 RESPONSE REQUIREMENT: Your recommendation field MUST contain either "BID" or "NO BID" - be explicit and extremely conservative.

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
            .model("claude-sonnet-4-20250514")
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
        
        // LOG THE COMPLETE RESPONSE FOR DEBUGGING
        info!("🔬 === FULL CLAUDE RESPONSE START (resource_id: {}) ===", resource_id);
        info!("{}", response);
        info!("🔬 === FULL CLAUDE RESPONSE END (resource_id: {}) ===", resource_id);
        
        // Also log response metadata
        info!("📊 Response metadata:");
        info!("   Length: {} bytes", response.len());
        info!("   Starts with: '{}'", response.chars().take(20).collect::<String>());
        info!("   Ends with: '{}'", response.chars().rev().take(20).collect::<String>().chars().rev().collect::<String>());
        info!("   Contains JSON markers: starts_with_brace={}, ends_with_brace={}", 
              response.trim().starts_with('{'), response.trim().ends_with('}'));
        info!("   Contains ```json: {}", response.contains("```json"));
        info!("   Contains ```: {}", response.contains("```"));
        
        // Safe string truncation that respects UTF-8 character boundaries
        let preview = Self::safe_truncate(&response, 500);
        info!("📝 Raw Claude response (first 500 chars): {}", preview);
        
        // Try to extract JSON from response - Claude sometimes wraps JSON in text
        let json_str = Self::extract_json_from_response(&response);
        
        // Log the extraction attempt
        info!("🔧 JSON extraction attempt:");
        info!("   Original length: {}", response.len());
        info!("   Extracted length: {}", json_str.len());
        info!("   Same as original: {}", response == json_str);
        if response != json_str {
            info!("🔬 === EXTRACTED JSON START ===");
            info!("{}", json_str);
            info!("🔬 === EXTRACTED JSON END ===");
        }
        
        // Try to parse as JSON first
        info!("🔧 Attempting to parse extracted JSON...");
        match serde_json::from_str::<Value>(&json_str) {
            Ok(json_response) => {
                info!("✅ Successfully parsed Claude response as JSON");
                
                // Log the structure of the parsed JSON
                info!("🏗️ JSON structure analysis:");
                info!("   Has 'summary' field: {}", json_response.get("summary").is_some());
                info!("   Has 'key_points' field: {}", json_response.get("key_points").is_some());
                info!("   Has 'recommendation' field: {}", json_response.get("recommendation").is_some());
                info!("   Has 'confidence_assessment' field: {}", json_response.get("confidence_assessment").is_some());
                
                // Log all top-level keys
                if let Some(obj) = json_response.as_object() {
                    let keys: Vec<&String> = obj.keys().collect();
                    info!("   All JSON keys: {:?}", keys);
                }
                
                let summary = json_response["summary"].as_str().unwrap_or(&response).to_string();
                let key_points = json_response["key_points"]
                    .as_array()
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_else(|| vec!["AI response could not be fully parsed".to_string()]);
                let recommendation = json_response["recommendation"].as_str().unwrap_or("See summary").to_string();
                let confidence_assessment = json_response["confidence_assessment"].as_str().unwrap_or("Moderate confidence").to_string();
                
                info!("🎯 Parsed Claude data:");
                info!("   Summary: '{}'", summary);
                info!("   Key points: {:?}", key_points);
                info!("   Recommendation: '{}'", recommendation);
                info!("   Confidence: '{}'", confidence_assessment);
                
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
            },
            Err(parse_error) => {
                // Fallback: use entire response as summary
                warn!("⚠️ Could not parse Claude response as JSON");
                warn!("📄 JSON parsing error: {}", parse_error);
                warn!("📄 Attempted JSON extraction: {}", json_str);
                
                // Try to extract recommendation from plain text
                let extracted_recommendation = Self::extract_recommendation_from_text(&response);
                
                Ok(AISummaryResult {
                    resource_id,
                    summary_type: summary_type.to_string(),
                    ai_summary: response.clone(),
                    key_points: vec!["Claude response was in plain text format".to_string()],
                    recommendation: extracted_recommendation,
                    confidence_assessment: "Unknown - response format issue".to_string(),
                    processing_notes: vec!["Claude response could not be parsed as JSON".to_string()],
                    created_at: Utc::now(),
                })
            }
        }
    }
    
    /// Extract JSON from Claude response that might be wrapped in text
    fn extract_json_from_response(response: &str) -> String {
        info!("🔧 Attempting JSON extraction from response...");
        
        // First try the response as-is
        let trimmed = response.trim();
        if trimmed.starts_with('{') && trimmed.ends_with('}') {
            info!("   ✅ Response is already clean JSON (starts and ends with braces)");
            return trimmed.to_string();
        }
        
        // Look for JSON block markers (```json ... ```)
        if let Some(start_pos) = response.find("```json") {
            info!("   🔍 Found ```json marker at position {}", start_pos);
            let after_marker = start_pos + 7; // Skip "```json"
            
            // Skip any whitespace/newlines after ```json
            let content_start = response[after_marker..].chars()
                .position(|c| !c.is_whitespace())
                .map(|pos| after_marker + pos)
                .unwrap_or(after_marker);
            
            if let Some(end_pos) = response[content_start..].find("```") {
                let json_content = &response[content_start..content_start + end_pos];
                info!("   ✅ Extracted JSON from ```json block (content length: {})", json_content.len());
                info!("   📝 Extracted content starts with: '{}'", json_content.chars().take(50).collect::<String>());
                return json_content.trim().to_string();
            } else {
                info!("   ⚠️ Found ```json but no closing ```");
            }
        }
        
        // Look for just { } blocks (find the outermost braces)
        if let Some(start_pos) = response.find('{') {
            info!("   🔍 Found opening brace at position {}", start_pos);
            
            // Find the matching closing brace
            let mut brace_count = 0;
            let mut end_pos = None;
            
            for (i, c) in response[start_pos..].char_indices() {
                match c {
                    '{' => brace_count += 1,
                    '}' => {
                        brace_count -= 1;
                        if brace_count == 0 {
                            end_pos = Some(start_pos + i);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            
            if let Some(end) = end_pos {
                let json_content = &response[start_pos..=end];
                info!("   ✅ Extracted JSON from brace matching (content length: {})", json_content.len());
                info!("   📝 Extracted content starts with: '{}'", json_content.chars().take(50).collect::<String>());
                return json_content.to_string();
            } else {
                info!("   ⚠️ Found opening brace but no matching closing brace");
            }
        }
        
        // No JSON structure found
        info!("   ❌ No JSON structure detected in response");
        response.to_string()
    }
    
    /// Extract recommendation from plain text response
    fn extract_recommendation_from_text(text: &str) -> String {
        let text_lower = text.to_lowercase();
        
        // Look for explicit bid recommendations
        if text_lower.contains("recommend bid") || text_lower.contains("should bid") {
            return "BID".to_string();
        }
        
        if text_lower.contains("no bid") || text_lower.contains("don't bid") || text_lower.contains("do not bid") {
            return "NO BID".to_string();
        }
        
        // Look for positive IT indicators as fallback
        let it_indicators = [
            "legitimate it", "genuine it opportunity", "clear it consultancy",
            "this is an it", "solid it opportunity", "technical opportunity"
        ];
        
        if it_indicators.iter().any(|&indicator| text_lower.contains(indicator)) {
            return "BID - IT opportunity identified".to_string();
        }
        
        // Default fallback
        "Review the summary for recommendations".to_string()
    }
}
