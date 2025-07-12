-- Quick check to see if PDF processing is working
SELECT 
    COUNT(*) as total_pdf_records,
    COUNT(CASE WHEN processing_status = 'COMPLETED' THEN 1 END) as completed_records,
    MAX(extraction_timestamp) as latest_extraction,
    COUNT(CASE WHEN extraction_timestamp >= NOW() - INTERVAL '1 hour' THEN 1 END) as processed_last_hour
FROM pdf_content;

-- Show recent PDF processing activity
SELECT 
    resource_id,
    LENGTH(pdf_text) as text_length,
    codes_count,
    processing_status,
    extraction_timestamp
FROM pdf_content 
WHERE extraction_timestamp >= NOW() - INTERVAL '1 hour'
ORDER BY extraction_timestamp DESC
LIMIT 10;

-- Check if any records were processed today
SELECT 
    COUNT(*) as records_processed_today
FROM pdf_content 
WHERE DATE(extraction_timestamp) = CURRENT_DATE;
