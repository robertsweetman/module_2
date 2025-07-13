use crate::types::{TenderRecord, FeatureVector};
use anyhow::Result;
use regex::Regex;
use std::collections::HashMap;
use once_cell::sync::Lazy;

/// Feature extractor for tender records
/// 
/// Extracts the 14 key features identified as most important:
/// 1. codes_count - Most important predictor
/// 2. has_codes - Binary indicator  
/// 3. title_length - Text complexity
/// 4. ca_encoded - Contracting authority
/// 5-14. TF-IDF features for key terms
pub struct FeatureExtractor {
    term_patterns: Vec<Regex>,
}

/// Static key terms identified as most predictive for bids
static KEY_TERMS: &[&str] = &[
    "software", "support", "provision", "computer", "services",
    "systems", "management", "works", "package", "technical"
];

/// Common contracting authorities mapping for encoding
static CA_MAPPING: Lazy<HashMap<&'static str, u32>> = Lazy::new(|| {
    let mut map = HashMap::new();
    map.insert("Health Service Executive", 1);
    map.insert("Dublin City Council", 2);
    map.insert("Cork City Council", 3);
    map.insert("Galway City Council", 4);
    map.insert("Department of Education", 5);
    map.insert("Department of Health", 6);
    map.insert("Office of Public Works", 7);
    map.insert("Transport Infrastructure Ireland", 8);
    map.insert("Irish Water", 9);
    map.insert("Revenue Commissioners", 10);
    // Add more as needed, unknown CAs will get value 0
    map
});

impl FeatureExtractor {
    /// Create new feature extractor
    pub fn new() -> Self {
        // Pre-compile regex patterns for efficiency
        let term_patterns = KEY_TERMS
            .iter()
            .map(|term| Regex::new(&format!(r"(?i)\b{}\b", regex::escape(term))))
            .collect::<Result<Vec<_>, _>>()
            .expect("Failed to compile regex patterns");

        Self {
            term_patterns,
        }
    }
    
    /// Extract feature vector from tender record
    pub fn extract_features(&self, tender: &TenderRecord) -> Result<FeatureVector> {
        // 1. codes_count (most important feature)
        // 1. codes_count - use the count provided by pdf_processing, or 0 if not available
        let codes_count = tender.codes_count.unwrap_or(0) as f64;
        
        // 2. has_codes (binary indicator)
        let has_codes = if codes_count > 0.0 { 1.0 } else { 0.0 };
        
        // 3. title_length
        let title_length = tender.title.len() as f64;
        
        // 4. ca_encoded (contracting authority)
        let ca_encoded = self.encode_contracting_authority(&tender.contracting_authority);
        
        // 5-14. TF-IDF features for key terms
        let combined_text = format!(
            "{} {}",
            tender.title,
            tender.pdf_content.as_ref().unwrap_or(&String::new())
        ).to_lowercase();
        
        let tfidf_features = self.calculate_tfidf_features(&combined_text)?;
        
        Ok(FeatureVector {
            codes_count,
            has_codes,
            title_length,
            ca_encoded,
            tfidf_software: tfidf_features[0],
            tfidf_support: tfidf_features[1],
            tfidf_provision: tfidf_features[2],
            tfidf_computer: tfidf_features[3],
            tfidf_services: tfidf_features[4],
            tfidf_systems: tfidf_features[5],
            tfidf_management: tfidf_features[6],
            tfidf_works: tfidf_features[7],
            tfidf_package: tfidf_features[8],
            tfidf_technical: tfidf_features[9],
        })
    }
    
    /// Encode contracting authority to numeric value
    fn encode_contracting_authority(&self, ca: &str) -> f64 {
        // Check if exact match in static mapping
        if let Some(&code) = CA_MAPPING.get(ca) {
            return code as f64;
        }
        
        // Check for partial matches for common variations
        for (pattern, &code) in CA_MAPPING.iter() {
            if ca.contains(pattern) || pattern.contains(ca) {
                return code as f64;
            }
        }
        
        // Use hash-based encoding for unknown CAs
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        ca.hash(&mut hasher);
        let hash_value = hasher.finish();
        
        // Map to reasonable range (11-100) to avoid conflicts with known mappings
        ((hash_value % 90) + 11) as f64
    }
    
    /// Calculate TF-IDF features for key terms
    fn calculate_tfidf_features(&self, text: &str) -> Result<Vec<f64>> {
        let mut features = Vec::with_capacity(KEY_TERMS.len());
        
        // Word count for normalization
        let word_count = text.split_whitespace().count() as f64;
        if word_count == 0.0 {
            return Ok(vec![0.0; KEY_TERMS.len()]);
        }
        
        for pattern in &self.term_patterns {
            // Count occurrences of the term
            let matches = pattern.find_iter(text).count() as f64;
            
            // Calculate TF (term frequency)
            let tf = matches / word_count;
            
            // Simplified IDF calculation (in production, this would use corpus statistics)
            // For now, we use a simplified approach based on term importance
            let idf = self.get_term_idf_weight(&pattern.as_str());
            
            // TF-IDF score
            let tfidf = tf * idf;
            features.push(tfidf.min(1.0)); // Cap at 1.0 for normalization
        }
        
        Ok(features)
    }
    
    /// Get IDF weight for term (simplified - in production would be calculated from corpus)
    fn get_term_idf_weight(&self, _term_pattern: &str) -> f64 {
        // Simplified IDF weights based on analysis results
        // Higher weights for terms that are more discriminative for bids
        match _term_pattern {
            pattern if pattern.contains("software") => 2.5,
            pattern if pattern.contains("support") => 2.0,
            pattern if pattern.contains("computer") => 1.8,
            pattern if pattern.contains("technical") => 1.5,
            pattern if pattern.contains("services") => 1.3,
            pattern if pattern.contains("systems") => 1.2,
            _ => 1.0, // Default weight for other terms
        }
    }   
}

impl Default for FeatureExtractor {
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
            resource_id: 123,
            title: "Software Development and Technical Support Services".to_string(),
            contracting_authority: "Health Service Executive".to_string(),
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
            pdf_content: Some("We require comprehensive software development services including technical support and computer systems management.".to_string()),
            detected_codes: Some(vec!["72000000".to_string(), "72200000".to_string(), "72600000".to_string()]),
            codes_count: Some(3), // Test with 3 detected codes
            processing_stage: Some("ml_prediction".to_string()),
            ml_bid: None,
            ml_confidence: None,
            ml_reasoning: None,
        }
    }

    #[test]
    fn test_feature_extraction() {
        let extractor = FeatureExtractor::new();
        let tender = create_test_tender();
        
        let features = extractor.extract_features(&tender).unwrap();
        
        assert_eq!(features.codes_count, 3.0);
        assert_eq!(features.has_codes, 1.0);
        assert!(features.title_length > 0.0);
        assert!(features.ca_encoded > 0.0);
        
        // Should detect software-related terms
        assert!(features.tfidf_software > 0.0);
        assert!(features.tfidf_support > 0.0);
        assert!(features.tfidf_technical > 0.0);
    }

    #[test]
    fn test_ca_encoding() {
        let extractor = FeatureExtractor::new();
        
        // Known CA should get specific code
        let hse_code = extractor.encode_contracting_authority("Health Service Executive");
        assert_eq!(hse_code, 1.0);
        
        // Unknown CA should get hash-based code
        let unknown_code = extractor.encode_contracting_authority("Unknown Authority");
        assert!(unknown_code >= 11.0 && unknown_code <= 100.0);
    }

    #[test]
    fn test_tfidf_calculation() {
        let extractor = FeatureExtractor::new();
        let text = "software development technical support computer systems";
        
        let features = extractor.calculate_tfidf_features(text).unwrap();
        
        assert_eq!(features.len(), KEY_TERMS.len());
        assert!(features[0] > 0.0); // software
        assert!(features[1] > 0.0); // support
        assert!(features[3] > 0.0); // computer
        assert!(features[5] > 0.0); // systems
        assert!(features[9] > 0.0); // technical
    }

    #[test]
    fn test_empty_text_handling() {
        let extractor = FeatureExtractor::new();
        let features = extractor.calculate_tfidf_features("").unwrap();
        
        assert_eq!(features.len(), KEY_TERMS.len());
        assert!(features.iter().all(|&f| f == 0.0));
    }
}
