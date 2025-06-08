use pdf_processing::extract_codes;
use std::fs;
use std::path::{Path, PathBuf};
use std::env;

#[test]
fn test_code_extraction() {
    // Sample text with codes
    let text = "This document includes requirements for IT services (72000000) 
                including Software programming and consultancy services (72200000).
                We're looking for Internet services (72400000) as well.";
    
    // Get current directory for debugging
    let current_dir = env::current_dir().unwrap();
    println!("Current directory: {:?}", current_dir);

    // Try to find codes.txt
    let mut codes_path = PathBuf::new();
    let possible_relative_paths = [
        "../codes.txt",              // From tests dir
        "codes.txt",                 // Current dir
        "../pdf_processing/codes.txt", // From tests dir to pdf_processing dir
    ];
    
    for rel_path in &possible_relative_paths {
        let path = Path::new(rel_path);
        println!("Checking path: {:?}", path);
        
        if path.exists() {
            codes_path = path.to_path_buf();
            println!("Found codes file at: {:?}", codes_path);
            break;
        }
    }
    
    if codes_path.as_os_str().is_empty() {
        panic!("Could not find codes.txt file in any expected location");
    }

    // Load real codes
    let codes_text = fs::read_to_string(&codes_path).expect("Failed to read codes file");
    let codes: Vec<String> = codes_text.lines().map(|s| s.to_string()).collect();
    
    // Test code extraction
    let found_codes = extract_codes(text, &codes);
    
    // Assertions
    assert!(!found_codes.is_empty(), "Should detect at least one code");
    assert!(found_codes.iter().any(|c| c.contains("72000000")), "Should detect 72000000");
    assert!(found_codes.iter().any(|c| c.contains("72200000")), "Should detect 72200000");
    assert!(found_codes.iter().any(|c| c.contains("72400000")), "Should detect 72400000");
    assert_eq!(found_codes.len(), 3, "Should detect exactly 3 codes");
}
