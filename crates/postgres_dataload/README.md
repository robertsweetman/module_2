# Running local dataload test

In a new window navigate to the crate you want to test and run `cargo lambda watch`

Generally use WSL for rust development to avoid (my) ARM64 Windows compilation issues.

```bash
wsl bash -c "source ~/.cargo/env && cd /mnt/c/Users/rober/GitHub/module_2/crates/[crate_name] && cargo build"
```

Running the postgres_dataload function in AWS Lambda

## Test Mode (for development)
```json
{
  "test_mode": true,
  "max_pages": 1
}
```

## Load recent tenders (since June 19th - about 53 new pages)
### Option 1: Full load (may overwhelm PDF queue)
```json
{
  "start_page": 1,
  "max_pages": 5,
  "test_mode": false
}
```

### Option 2: Chunked processing (recommended - 5 pages at a time)
Run these sequentially to avoid PDF processing queue issues:

**Chunk 1 (pages 1-5):**
```json
{
  "start_page": 1,
  "max_pages": 5,
  "test_mode": false
}
```

**Chunk 2 (pages 6-10):**
```json
{
  "start_page": 6,
  "max_pages": 5,
  "test_mode": false
}
```

**Chunk 3 (pages 11-15):**
```json
{
  "start_page": 11,
  "max_pages": 5,
  "test_mode": false
}
```

**Continue pattern up to page 53...**

## Load specific page range
```json
{
  "start_page": 1,
  "max_pages": 10,
  "test_mode": false
}
```

**Parameters:**
- `test_mode`: If true, only processes 5 records and returns sample data (no DB save)
- `offset`: Goes back N pages from page 1 (use this to get recent data)
- `start_page`: Which page to start from (default: 1)
- `max_pages`: How many pages to process (default: 10)

**Note:** The lambda automatically queues PDFs for processing via SQS when not in test mode.

## Chunked Processing Strategy

When processing 53 new pages, break into chunks of 5 to avoid overwhelming the PDF processing queue:

### Progress Tracking
- **Total pages to process:** 53 (pages 1-53)
- **Chunk size:** 5 pages
- **Total chunks:** 11 chunks (10 chunks of 5 pages + 1 chunk of 3 pages)
- **Records per page:** 10 tenders per page = ~50 tenders per chunk

### Recommended Workflow
1. Run **Quick Status Check** (see PDF validation section below)
2. Process one chunk via AWS Lambda
3. Wait 5-10 minutes for PDF processing to complete
4. Run **Quick Status Check** again to verify processing
5. Check for any PDF processing failures
6. Proceed to next chunk

### All Chunk Configurations
```json
// Chunk 1:  {"start_page": 1,  "max_pages": 5, "test_mode": false}  // Pages 1-5
// Chunk 2:  {"start_page": 6,  "max_pages": 5, "test_mode": false}  // Pages 6-10
// Chunk 3:  {"start_page": 11, "max_pages": 5, "test_mode": false}  // Pages 11-15
// Chunk 4:  {"start_page": 16, "max_pages": 5, "test_mode": false}  // Pages 16-20
// Chunk 5:  {"start_page": 21, "max_pages": 5, "test_mode": false}  // Pages 21-25
// Chunk 6:  {"start_page": 26, "max_pages": 5, "test_mode": false}  // Pages 26-30
// Chunk 7:  {"start_page": 31, "max_pages": 5, "test_mode": false}  // Pages 31-35
// Chunk 8:  {"start_page": 36, "max_pages": 5, "test_mode": false}  // Pages 36-40
// Chunk 9:  {"start_page": 41, "max_pages": 5, "test_mode": false}  // Pages 41-45
// Chunk 10: {"start_page": 46, "max_pages": 5, "test_mode": false}  // Pages 46-50
// Chunk 11: {"start_page": 51, "max_pages": 3, "test_mode": false}  // Pages 51-53
```

## PDF Processing Validation

Before and after running dataload, use these SQL queries to validate PDF processing status:

### Quick Status Check
```sql
-- Current state summary
SELECT 
    COUNT(*) as total_records,
    MAX(published) as latest_published,
    COUNT(CASE WHEN pdf_url IS NOT NULL AND pdf_url != '' THEN 1 END) as records_with_pdf,
    COUNT(CASE WHEN pdf_url IS NOT NULL AND pdf_url != '' 
               AND NOT EXISTS (SELECT 1 FROM pdf_content pc WHERE pc.resource_id = tender_records.resource_id) 
          THEN 1 END) as pending_pdf_processing
FROM tender_records
WHERE published >= '2025-06-19';
```

### Find Records Needing PDF Processing
```sql
-- Tenders with PDFs that haven't been processed yet
SELECT 
    tr.resource_id,
    tr.title,
    tr.ca,
    tr.published,
    tr.pdf_url
FROM tender_records tr
LEFT JOIN pdf_content pc ON tr.resource_id = pc.resource_id
WHERE tr.pdf_url IS NOT NULL 
    AND tr.pdf_url != ''
    AND pc.resource_id IS NULL
    AND tr.published >= '2025-06-19'
ORDER BY tr.published DESC
LIMIT 10;
```

For complete PDF validation queries, see: `../../pdf_validation_queries.md`