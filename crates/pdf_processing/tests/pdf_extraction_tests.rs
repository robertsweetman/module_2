// pub use crate::main::extract_text_from_pdf;

use pdf_processing::extract_text_from_pdf;
use reqwest;
use std::fs;

#[tokio::test]
async fn test_pdf_download_and_extraction() -> Result<(), Box<dyn std::error::Error>> {
    // Download PDF
    let pdf_url = "https://www.etenders.gov.ie/epps/cft/downloadNoticeForAdvSearch.do?resourceId=5850990";
    println!("Downloading PDF from: {}", pdf_url);
    
    let client = reqwest::Client::new();
    let response = client.get(pdf_url).send().await?;
    
    if !response.status().is_success() {
        panic!("Failed to download PDF: HTTP {}", response.status());
    }
    
    let pdf_bytes = response.bytes().await?;
    println!("Downloaded {} bytes", pdf_bytes.len());
    
    // Save PDF locally for inspection
    fs::write("test.pdf", &pdf_bytes)?;
    
    // Use the function being tested
    let text = extract_text_from_pdf(&pdf_bytes)?;
    
    // Save extracted text for inspection
    fs::write("test.txt", &text)?;
    
    // Assertions
    assert!(!text.is_empty(), "Extracted text should not be empty");
    assert!(text.len() > 100, "Text should be substantial (got {} chars)", text.len());
    println!("Successfully extracted {} characters", text.len());
    
    Ok(())
}
