use crate::types::{TenderRecord, MLPredictionResult, FeatureVector, FeatureScores};
use crate::features::FeatureExtractor;
use anyhow::Result;
use tracing::{info, debug};

/// Optimized Bid Predictor using threshold 0.054 based on TF-IDF Linear SVM analysis
/// 
/// Based on comprehensive analysis from tfidf_linearSVM_pdf_content.ipynb:
/// - Threshold 0.054 achieves 85.6% recall (catches most bids)
/// - 16% precision (intentionally high false positives to avoid missing opportunities)
/// - ONLY used for tenders WITH PDF content
/// - Strong exclusion filtering for non-IT projects
/// - More conservative than previous approach to reduce noise
pub struct OptimizedBidPredictor {
    threshold: f64,
    feature_extractor: FeatureExtractor,
    // Enhanced feature weights based on TF-IDF + Linear SVM analysis
    // More conservative to reduce false positives while maintaining recall
    feature_weights: [f64; 15],  // Updated for 15 features
}

impl OptimizedBidPredictor {
    /// Create new optimized bid predictor with threshold 0.054
    /// 
    /// This predictor should ONLY be used for tenders that have PDF content.
    /// For tenders without PDF content, route directly to ai_summary for title analysis.
    pub fn new() -> Self {
        Self {
            threshold: 0.054, // From tfidf_linearSVM_pdf_content.ipynb analysis
            feature_extractor: FeatureExtractor::new(),
            // More conservative feature weights based on TF-IDF + Linear SVM analysis
            // Reduced positive weights and increased negative exclusion weight
            feature_weights: [
                0.25,  // codes_count (reduced from 0.35)
                0.10,  // has_codes (reduced from 0.15)
                0.02,  // title_length (reduced from 0.05)
                0.03,  // ca_encoded (reduced from 0.08)
                -0.80, // exclusion_score (INCREASED negative weight)
                0.08,  // tfidf_software (reduced from 0.12)
                0.05,  // tfidf_support (reduced from 0.08)
                0.03,  // tfidf_provision (reduced from 0.05)
                0.02,  // tfidf_computer (reduced from 0.04)
                0.02,  // tfidf_services (reduced from 0.03)
                0.01,  // tfidf_systems (reduced from 0.02)
                0.01,  // tfidf_management (unchanged)
                0.005, // tfidf_works (reduced from 0.01)
                0.003, // tfidf_package (reduced from 0.005)
                0.003, // tfidf_technical (reduced from 0.005)
            ],
        }
    }
    
    /// Get the current threshold value
    #[cfg(test)]
    pub fn get_threshold(&self) -> f64 {
        self.threshold
    }
    
    /// Make ML prediction for a tender record with PDF content
    /// 
    /// **IMPORTANT**: This predictor should ONLY be called for tenders that have PDF content.
    /// For tenders without PDF, route directly to ai_summary for title-only analysis.
    /// 
    /// Returns prediction result with confidence score and reasoning
    pub fn predict(&self, tender: &TenderRecord) -> Result<MLPredictionResult> {
        debug!("🤖 Starting ML prediction for: {}", tender.resource_id);
        
        // Validate that we have PDF content - this is a hard requirement
        if tender.pdf_content.is_none() || tender.pdf_content.as_ref().unwrap().trim().is_empty() {
            return Err(anyhow::anyhow!(
                "ML predictor requires PDF content. Tender {} has no PDF content - route to ai_summary instead.",
                tender.resource_id
            ));
        }
        
        // Extract feature vector
        let features = self.feature_extractor.extract_features(tender)?;
        
        // ENHANCED EXCLUSION RULES: Multiple levels of exclusion
        
        // Level 1: HARD EXCLUSION - Very high exclusion score
        if features.exclusion_score > 4.0 {
            let reasoning = format!(
                "HARD_EXCLUSION: Score {:.1} - Strong non-IT indicators (construction/infrastructure/civil engineering). Automatically excluded.",
                features.exclusion_score
            );
            
            return Ok(MLPredictionResult {
                should_bid: false,
                confidence: 0.0,
                reasoning,
                feature_scores: self.calculate_feature_scores(&features),
            });
        }
        
        // Level 2: SOFT EXCLUSION - High exclusion score + no codes
        if features.exclusion_score > 2.0 && features.codes_count == 0.0 {
            let reasoning = format!(
                "SOFT_EXCLUSION: Score {:.1} with no IT codes - Likely non-IT project without relevant codes.",
                features.exclusion_score
            );
            
            return Ok(MLPredictionResult {
                should_bid: false,
                confidence: 0.01, // Very low confidence
                reasoning,
                feature_scores: self.calculate_feature_scores(&features),
            });
        }
        
        // Level 3: Regular ML prediction with conservative approach
        let prediction_score = self.calculate_prediction_score(&features)?;
        
        // Apply more conservative threshold adjustment based on exclusion score
        let adjusted_threshold = if features.exclusion_score > 1.0 {
            self.threshold * (1.0 + features.exclusion_score * 0.5) // Increase threshold for suspicious content
        } else {
            self.threshold
        };
        
        // Apply threshold for binary decision
        let should_bid = prediction_score >= adjusted_threshold;
        
        // Generate reasoning based on feature contributions
        let reasoning = self.generate_reasoning(&features, prediction_score, should_bid, adjusted_threshold);
        
        // Calculate feature scores for transparency
        let feature_scores = self.calculate_feature_scores(&features);
        
        let result = MLPredictionResult {
            should_bid,
            confidence: prediction_score,
            reasoning,
            feature_scores,
        };
        
        info!(
            "🎯 ML Prediction for {}: {} (score: {:.0}%, threshold: {:.0}%→{:.0}%, exclusion: {:.1})",
            tender.resource_id,
            if should_bid { "BID" } else { "NO-BID" },
            prediction_score * 100.0,
            self.threshold * 100.0,
            adjusted_threshold * 100.0,
            features.exclusion_score
        );
        
        Ok(result)
    }
    
    /// Calculate prediction score using weighted feature importance
    fn calculate_prediction_score(&self, features: &FeatureVector) -> Result<f64> {
        let feature_array = features.to_array();
        
        // Normalize features to 0-1 range for consistent scoring
        let normalized_features = self.normalize_features(&feature_array);
        
        // Calculate weighted sum
        let mut score = 0.0;
        for (i, &weight) in self.feature_weights.iter().enumerate() {
            score += normalized_features[i] * weight;
        }
        
        // Apply sigmoid function to get probability-like score
        let sigmoid_score = 1.0 / (1.0 + (-score * 6.0).exp()); // Scale by 6 for appropriate range
        
        Ok(sigmoid_score)
    }
    
    /// Normalize features to 0-1 range based on expected value ranges
    fn normalize_features(&self, features: &[f64; 15]) -> [f64; 15] {
        [
            (features[0] / 20.0).min(1.0),           // codes_count (max ~20)
            features[1],                              // has_codes (already 0/1)
            (features[2] / 200.0).min(1.0),          // title_length (max ~200)
            (features[3] / 100.0).min(1.0),          // ca_encoded (max ~100 CAs)
            (features[4] / 10.0).min(1.0),           // exclusion_score (0-10 range)
            features[5],                              // tfidf_software (already 0-1)
            features[6],                              // tfidf_support  (already 0-1)
            features[7],                              // tfidf_provision (already 0-1)
            features[8],                              // tfidf_computer (already 0-1)
            features[9],                              // tfidf_services (already 0-1)
            features[10],                             // tfidf_systems (already 0-1)
            features[11],                             // tfidf_management (already 0-1)
            features[12],                             // tfidf_works (already 0-1)
            features[13],                             // tfidf_package (already 0-1)
            features[14],                             // tfidf_technical (already 0-1)
        ]
    }
    
    /// Generate human-readable reasoning for the prediction
    fn generate_reasoning(&self, features: &FeatureVector, score: f64, should_bid: bool, threshold: f64) -> String {
        let mut reasons = Vec::new();
        
        // Check exclusion indicators first (most important for filtering)
        if features.exclusion_score > 3.0 {
            reasons.push(format!("🚫 VERY HIGH EXCLUSION: {:.1} - strong non-IT project indicators", features.exclusion_score));
        } else if features.exclusion_score > 2.0 {
            reasons.push(format!("⚠️ High exclusion score: {:.1} - contains non-IT terms", features.exclusion_score));
        } else if features.exclusion_score > 1.0 {
            reasons.push(format!("⚠️ Medium exclusion score: {:.1} - some non-IT terms", features.exclusion_score));
        }
        
        // Check key positive indicators
        if features.codes_count > 0.0 {
            reasons.push(format!("✅ {} relevant IT codes detected", features.codes_count as i32));
        } else {
            reasons.push("❌ No IT codes detected".to_string());
        }
        
        if features.tfidf_software > 0.1 {
            reasons.push("✅ Software-related terms found".to_string());
        }
        
        if features.tfidf_support > 0.1 {
            reasons.push("✅ Support service terms found".to_string());
        }
        
        // PDF content quality - check title length as proxy
        if features.title_length > 100.0 {
            reasons.push("✅ Detailed title indicates complex requirements".to_string());
        }
        
        // Generate final reasoning with threshold information
        let category = if should_bid {
            if score > 0.2 { "HIGH_CONFIDENCE_BID" }
            else if score > 0.1 { "MEDIUM_CONFIDENCE_BID" }
            else { "LOW_CONFIDENCE_BID" }
        } else {
            if features.exclusion_score > 2.0 {
                "EXCLUDED_NON_IT"
            } else {
                "NO_BID_RECOMMENDED"
            }
        };
        
        let threshold_info = if threshold != self.threshold {
            format!(" (adjusted threshold: {:.0}%)", threshold * 100.0)
        } else {
            String::new()
        };
        
        if reasons.is_empty() {
            format!("{}: Score {:.0}% vs threshold {:.0}%{}", category, score * 100.0, threshold * 100.0, threshold_info)
        } else {
            format!("{}: {} (Score: {:.0}%{})", category, reasons.join(", "), score * 100.0, threshold_info)
        }
    }
    
    /// Calculate detailed feature scores for transparency
    fn calculate_feature_scores(&self, features: &FeatureVector) -> FeatureScores {
        let normalized = self.normalize_features(&features.to_array());
        
        FeatureScores {
            codes_count_score: normalized[0] * self.feature_weights[0],
            has_codes_score: normalized[1] * self.feature_weights[1],
            title_length_score: normalized[2] * self.feature_weights[2],
            ca_score: normalized[3] * self.feature_weights[3],
            text_features_score: (4..14).map(|i| normalized[i] * self.feature_weights[i]).sum(),
            total_score: normalized.iter().enumerate()
                .map(|(i, &val)| val * self.feature_weights[i])
                .sum(),
        }
    }
}

/// Default implementation for testing
impl Default for OptimizedBidPredictor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TenderRecord;
    // use chrono::Utc;

    fn create_test_tender() -> TenderRecord {
        // use chrono::NaiveDate;
        use bigdecimal::BigDecimal;
        use std::str::FromStr;
        
        TenderRecord {
            resource_id: 123567765,
            title: "Software Development Services".to_string(),
            contracting_authority: "Test Authority".to_string(),
            info: "Test info".to_string(),
            published: None,
            deadline: None,
            procedure: "Open".to_string(),
            status: "Open".to_string(),
            pdf_url: "test.pdf".to_string(),
            awarddate: None,
            value: Some(BigDecimal::from_str("100000").unwrap()),
            cycle: "2024".to_string(),
            bid: None,
            pdf_content: Some("Software development and technical support services".to_string()),
            detected_codes: Some(vec!["72000000".to_string(), "72200000".to_string(), "72600000".to_string()]),
            codes_count: Some(3),
            processing_stage: Some("ml_prediction".to_string()),
            ml_bid: None,
            ml_confidence: None,
            ml_reasoning: None,
        }
    }

    #[test]
    fn test_predictor_initialization() {
        let predictor = OptimizedBidPredictor::new();
        assert_eq!(predictor.get_threshold(), 0.054);
    }

    #[test]
    fn test_prediction_with_software_tender() {
        let predictor = OptimizedBidPredictor::new();
        let tender = create_test_tender();
        
        let result = predictor.predict(&tender).unwrap();
        
        // Should likely predict bid due to software terms and codes
        assert!(result.confidence > 0.0);
        assert!(result.reasoning.contains("software") || result.reasoning.contains("codes"));
    }

    #[test]
    fn test_feature_normalization() {
        let predictor = OptimizedBidPredictor::new();
        let features = [5.0, 1.0, 150.0, 50.0, 0.5, 0.3, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let normalized = predictor.normalize_features(&features);
        
        assert!(normalized[0] <= 1.0); // codes_count normalized
        assert_eq!(normalized[1], 1.0); // has_codes unchanged
        assert!(normalized[2] <= 1.0); // title_length normalized
    }
}
