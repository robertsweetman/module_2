# PDF Processing Validation Queries

Use these SQL queries to validate PDF processing status in your PostgreSQL database.

## 0. Simple check for today's activity

```sql
-- Records created today
SELECT *
FROM tender_records 
WHERE DATE(created_at) = CURRENT_DATE
ORDER BY created_at DESC;
```

```sql
-- PDF content processed today
SELECT *
FROM pdf_content
WHERE DATE(extraction_timestamp) = CURRENT_DATE
ORDER BY extraction_timestamp DESC;
```

```sql
-- Today's records with PDF status
SELECT 
    tr.resource_id,
    tr.title,
    tr.ca,
    tr.pdf_url,
    tr.created_at,
    CASE 
        WHEN tr.pdf_url IS NULL OR tr.pdf_url = '' THEN 'No PDF URL'
        WHEN pc.resource_id IS NULL THEN 'PDF Not Processed'
        ELSE pc.processing_status
    END as pdf_status,
    pc.extraction_timestamp,
    pc.codes_count
FROM tender_records tr
LEFT JOIN pdf_content pc ON tr.resource_id = pc.resource_id
WHERE DATE(tr.created_at) = CURRENT_DATE
ORDER BY tr.created_at DESC;
```

## 1. Count tenders with and without PDFs

```sql
-- Overall PDF availability status
SELECT 
    COUNT(*) as total_tenders,
    COUNT(CASE WHEN pdf_url IS NOT NULL AND pdf_url != '' THEN 1 END) as tenders_with_pdf_url,
    COUNT(CASE WHEN pdf_url IS NULL OR pdf_url = '' THEN 1 END) as tenders_without_pdf_url,
    ROUND(
        (COUNT(CASE WHEN pdf_url IS NOT NULL AND pdf_url != '' THEN 1 END) * 100.0 / COUNT(*)), 2
    ) as pdf_availability_percentage
FROM tender_records;
```

## 2. Check PDF processing status

```sql
-- PDF processing completion status
SELECT 
    tr.resource_id,
    tr.title,
    tr.ca,
    tr.published,
    tr.deadline,
    CASE 
        WHEN tr.pdf_url IS NULL OR tr.pdf_url = '' THEN 'No PDF URL'
        WHEN pc.resource_id IS NULL THEN 'PDF Not Processed'
        WHEN pc.processing_status = 'success' THEN 'Successfully Processed'
        WHEN pc.processing_status = 'failed' THEN 'Processing Failed'
        ELSE pc.processing_status
    END as pdf_status,
    pc.extraction_timestamp,
    pc.codes_count,
    LENGTH(pc.pdf_text) as pdf_text_length
FROM tender_records tr
LEFT JOIN pdf_content pc ON tr.resource_id = pc.resource_id
WHERE tr.published >= '2025-06-19'  -- Since June 19th
ORDER BY tr.published DESC;
```

## 3. Summary of PDF processing by status

```sql
-- Summary statistics for PDF processing
SELECT 
    CASE 
        WHEN tr.pdf_url IS NULL OR tr.pdf_url = '' THEN 'No PDF URL'
        WHEN pc.resource_id IS NULL THEN 'PDF Not Processed'
        WHEN pc.processing_status = 'success' THEN 'Successfully Processed'
        WHEN pc.processing_status = 'failed' THEN 'Processing Failed'
        ELSE COALESCE(pc.processing_status, 'Unknown')
    END as status,
    COUNT(*) as count,
    ROUND((COUNT(*) * 100.0 / SUM(COUNT(*)) OVER()), 2) as percentage
FROM tender_records tr
LEFT JOIN pdf_content pc ON tr.resource_id = pc.resource_id
WHERE tr.published >= '2025-06-19'  -- Since June 19th
GROUP BY 
    CASE 
        WHEN tr.pdf_url IS NULL OR tr.pdf_url = '' THEN 'No PDF URL'
        WHEN pc.resource_id IS NULL THEN 'PDF Not Processed'
        WHEN pc.processing_status = 'success' THEN 'Successfully Processed'
        WHEN pc.processing_status = 'failed' THEN 'Processing Failed'
        ELSE COALESCE(pc.processing_status, 'Unknown')
    END
ORDER BY count DESC;
```

## 4. Find tenders that need PDF processing

```sql
-- Tenders with PDF URLs but not yet processed
SELECT 
    tr.resource_id,
    tr.title,
    tr.ca,
    tr.pdf_url,
    tr.published,
    tr.deadline
FROM tender_records tr
LEFT JOIN pdf_content pc ON tr.resource_id = pc.resource_id
WHERE tr.pdf_url IS NOT NULL 
    AND tr.pdf_url != ''
    AND pc.resource_id IS NULL
    AND tr.published >= '2025-06-19'  -- Since June 19th
ORDER BY tr.published DESC
LIMIT 20;
```

## 5. Check for processing failures

```sql
-- PDF processing failures with error details
SELECT 
    tr.resource_id,
    tr.title,
    tr.ca,
    tr.pdf_url,
    pc.processing_status,
    pc.extraction_timestamp,
    pc.pdf_text  -- This might contain error messages for failed processing
FROM tender_records tr
JOIN pdf_content pc ON tr.resource_id = pc.resource_id
WHERE pc.processing_status = 'failed'
    AND tr.published >= '2025-06-19'  -- Since June 19th
ORDER BY pc.extraction_timestamp DESC;
```

## 6. Check recent PDF processing activity

```sql
-- Recent PDF processing activity (last 24 hours)
SELECT 
    DATE_TRUNC('hour', pc.extraction_timestamp) as processing_hour,
    COUNT(*) as pdfs_processed,
    COUNT(CASE WHEN pc.processing_status = 'success' THEN 1 END) as successful,
    COUNT(CASE WHEN pc.processing_status = 'failed' THEN 1 END) as failed
FROM pdf_content pc
WHERE pc.extraction_timestamp >= NOW() - INTERVAL '24 hours'
GROUP BY DATE_TRUNC('hour', pc.extraction_timestamp)
ORDER BY processing_hour DESC;
```

## 7. Identify patterns in PDF processing issues

```sql
-- Common characteristics of tenders without PDF processing
SELECT 
    tr.ca as contracting_authority,
    COUNT(*) as total_tenders,
    COUNT(CASE WHEN tr.pdf_url IS NOT NULL AND tr.pdf_url != '' THEN 1 END) as with_pdf_url,
    COUNT(CASE WHEN pc.resource_id IS NOT NULL THEN 1 END) as processed_pdfs,
    ROUND(
        (COUNT(CASE WHEN pc.resource_id IS NOT NULL THEN 1 END) * 100.0 / 
         NULLIF(COUNT(CASE WHEN tr.pdf_url IS NOT NULL AND tr.pdf_url != '' THEN 1 END), 0)), 2
    ) as processing_rate_percentage
FROM tender_records tr
LEFT JOIN pdf_content pc ON tr.resource_id = pc.resource_id
WHERE tr.published >= '2025-06-19'  -- Since June 19th
GROUP BY tr.ca
HAVING COUNT(*) > 1  -- Only show authorities with multiple tenders
ORDER BY processing_rate_percentage ASC, total_tenders DESC;
```

## 8. Quick check before running dataload

```sql
-- Before running dataload - current state
SELECT 
    'Before Dataload' as checkpoint,
    COUNT(*) as total_records,
    MAX(published) as latest_published,
    COUNT(CASE WHEN pdf_url IS NOT NULL AND pdf_url != '' THEN 1 END) as records_with_pdf,
    COUNT(CASE WHEN pdf_url IS NOT NULL AND pdf_url != '' 
               AND NOT EXISTS (SELECT 1 FROM pdf_content pc WHERE pc.resource_id = tender_records.resource_id) 
          THEN 1 END) as pending_pdf_processing
FROM tender_records;
```

## Usage Instructions

1. **Before running dataload**: Run query #8 to establish baseline
2. **After running dataload**: Run query #8 again to see what was added
3. **Check processing status**: Use queries #2 and #3 to monitor PDF processing
4. **Find issues**: Use queries #4, #5, and #7 to identify problems

## Key Fields

- `tender_records.pdf_url`: URL to PDF document (empty if no PDF available)
- `pdf_content.processing_status`: 'success', 'failed', or other status
- `pdf_content.extraction_timestamp`: When PDF was processed
- `pdf_content.codes_count`: Number of procurement codes detected
- `pdf_content.pdf_text`: Extracted text content (or error message)
