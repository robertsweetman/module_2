pub fn extract_text_from_pdf(pdf_bytes: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
    let text = pdf_extract::extract_text_from_mem(pdf_bytes)?;
    Ok(text)
}

pub fn extract_codes(text: &str, codes: &[String]) -> Vec<String> {
    codes
        .iter()
        .filter(|code| text.contains(&code[..]))
        .cloned()
        .collect()
}