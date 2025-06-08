pub fn extract_text_from_pdf(pdf_bytes: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
    let text = pdf_extract::extract_text_from_mem(pdf_bytes)?;
    Ok(text)
}

pub fn extract_codes(text: &str, codes: &[String]) -> Vec<String> {
    let mut found_codes = Vec::new();
    
    for code in codes {
        // Check if the code or its description is in the text
        let parts: Vec<&str> = code.split(',').collect();
        if parts.len() == 2 {
            let code_number = parts[0].trim();
            let description = parts[1].trim();
            
            if text.contains(code_number) || text.contains(description) {
                found_codes.push(code.clone());
            }
        }
    }
    
    found_codes
}