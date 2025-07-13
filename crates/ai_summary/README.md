# AI Summary Lambda

This lambda processes messages from the `ai_summary_queue` and generates comprehensive AI summaries of tender opportunities using OpenAI's GPT-4 API.

## Processing Flow

1. **Receive SQS Message**: Gets `AISummaryMessage` from ai_summary_queue
2. **Strategy Selection**: Choose title-only or full PDF processing
3. **Database Fetch**: Get complete tender record and PDF content if needed  
4. **AI Processing**: Generate summary using OpenAI GPT-4
5. **Store Results**: Save summary to `ai_summaries` table
6. **Send Notification**: Publish completion notification to SNS topic

## Notifications

After successfully completing an AI summary, the lambda sends an SNS notification with:

- **Priority levels**:
  - `URGENT`: ML recommends bidding
  - `HIGH`: Full PDF analysis completed (no bid recommendation)
  - `NORMAL`: Title-only analysis completed

- **Notification content**:
  - Truncated summary (500 chars max)
  - Key metadata (value, deadline, contracting authority)
  - ML prediction results
  - Action required based on priority

- **SNS Message Structure**:
  ```json
  {
    "message_type": "AI_SUMMARY_COMPLETE",
    "resource_id": "12345",
    "title": "Software Development Services",
    "priority": "URGENT",
    "summary": "Truncated AI summary...",
    "action_required": "REVIEW IMMEDIATELY: ML recommends bidding",
    "timestamp": "2025-07-13T10:30:00Z",
    "metadata": {
      "contracting_authority": "Health Service Executive",
      "ml_prediction": {...},
      "key_points": [...],
      "recommendation": "...",
      // Additional context
    }
  }
  ```

## Processing Strategy

The lambda uses a two-tier processing strategy based on available content:

### 1. Title-Only Processing (Lightweight)
Used when:
- No PDF content is available
- PDF content is minimal (< 100 characters)

Process:
- Uses tender title and contracting authority
- Incorporates ML prediction results
- Generates quick assessment and recommendations

### 2. Full PDF Processing (Comprehensive)
Used when:
- Substantial PDF content is available OR
- PDF content can be fetched from the `pdf_content` table

Process:
- Fetches complete PDF text from database if needed
- Includes detected procurement codes
- Incorporates all tender metadata
- Generates detailed analysis with strategic recommendations

## Database Operations

The lambda performs the following database operations:

1. **Read from `pdf_content` table**: Fetches complete PDF text and detected codes
2. **Read from `tenders` table**: Gets complete tender record with metadata
3. **Write to `ai_summaries` table**: Stores the generated AI summary

### AI Summaries Table Schema

```sql
CREATE TABLE ai_summaries (
    resource_id BIGINT PRIMARY KEY,
    summary_type TEXT NOT NULL,           -- "TITLE_ONLY" or "FULL_PDF"
    ai_summary TEXT NOT NULL,             -- Main AI-generated summary
    key_points JSONB NOT NULL,            -- Array of key assessment points
    recommendation TEXT NOT NULL,         -- Strategic recommendation
    confidence_assessment TEXT NOT NULL,  -- Confidence in the assessment
    processing_notes JSONB NOT NULL,      -- Technical processing notes
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);
```

## Environment Variables

Required environment variables:

- `DATABASE_URL`: PostgreSQL connection string
- `OPENAI_API_KEY`: OpenAI API key for GPT-4 access
- `SNS_TOPIC_ARN`: SNS topic for notifications (future use)
- `AWS_REGION`: AWS region (defaults to eu-west-1)

## AI Processing

### Prompt Strategy

The lambda uses structured prompts that include:
- Tender details (title, authority, value, deadline)
- PDF content (truncated to 8000 chars if necessary)
- Detected procurement codes
- ML prediction results and reasoning

### Response Parsing

The AI service attempts to parse structured JSON responses with fields:
- `summary`: Executive summary
- `key_points`: Array of key assessment points
- `recommendation`: Strategic recommendation
- `confidence_assessment`: Confidence level and reasoning

If JSON parsing fails, the entire response is used as the summary.

## Error Handling

- Messages that fail processing don't cause the entire batch to fail
- Failed messages are sent to the `ai-summary-dlq` dead letter queue after 3 attempts
- Database connection issues are retried automatically
- OpenAI API errors are logged and result in processing failure

## Performance Considerations

- **Concurrency**: Limited to 3 concurrent executions to avoid OpenAI rate limits
- **Memory**: 512MB allocated for AI API calls and text processing
- **Timeout**: 5 minutes to handle API response delays
- **Database**: Uses connection pooling with max 5 connections

## Monitoring

Key metrics to monitor:
- Processing time per summary
- OpenAI API response times
- Database query performance
- Dead letter queue message count
- Cost per summary (OpenAI API usage)

## Cost Optimization

- PDF content is truncated to 8000 characters to stay within token limits
- GPT-4 temperature set to 0.3 for consistent, focused responses
- Efficient database queries to minimize connection time
- Batch processing disabled (batch_size=1) for optimal AI API usage
