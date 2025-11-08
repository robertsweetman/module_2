# Daily Tender Scan Setup Guide

## Overview

This guide explains how the tender scanning system works and how to set it up to run daily at 09:00 UTC.

## System Flow

```
1. get_data Lambda (scheduled 09:00 daily)
   ↓
2. Scans 10 pages from etenders.gov.ie
   ↓
3. Filters out duplicates (already in DB)
   ↓
4. Saves only NEW tenders to database
   ↓
5. Processes PDFs and extracts codes
   ↓
6. Sends to ML prediction queue
   ↓
7. ML predictor evaluates tender
   ↓
8. Sends to AI summary queue
   ↓
9. AI summary generates summary
   ↓
10. SNS notification sends email
    ↓
11. Marks tender as notified (prevents duplicates)
```

## Database Changes

### New Columns Added to `tender_records` Table

```sql
-- Track which tenders have been notified
notification_sent BOOLEAN DEFAULT FALSE
notification_sent_at TIMESTAMP WITH TIME ZONE DEFAULT NULL
```

These columns ensure:
- ✅ **No duplicate emails** - even if a tender is processed twice
- ✅ **Audit trail** - know exactly when each notification was sent
- ✅ **Idempotency** - safe to re-run the process

## Deduplication Strategy

### 1. **Initial Fetch** (get_data Lambda)
- Fetches 10 pages (~200 tenders)
- Checks each tender's `resource_id` against database
- **Only processes NEW tenders** that don't exist in DB
- Existing tenders are skipped entirely

**Code location:** `crates/get_data/src/main.rs`
```rust
async fn filter_new_records(
    pool: &Pool<Postgres>,
    records: &[TenderRecord],
) -> Result<Vec<TenderRecord>, Error>
```

### 2. **Notification Prevention** (sns_notification Lambda)
- After sending email, marks tender with:
  - `notification_sent = TRUE`
  - `notification_sent_at = NOW()`
- Even if ML re-processes a tender, no duplicate email is sent

**Code location:** `crates/sns_notification/src/main.rs`
```rust
async fn mark_tender_as_notified(pool: &PgPool, resource_id: i64) -> Result<()>
```

## Why Scan 10 Pages?

Looking at 10 pages (~200 tenders) ensures we:
- ✅ Don't miss any new tenders (even if publication delays occur)
- ✅ Catch tenders published overnight or on weekends
- ✅ Handle cases where multiple tenders are published at once
- ✅ Provide buffer for system downtime recovery

Since we **deduplicate at the database level**, processing existing tenders has minimal cost:
- Database check: ~1ms per tender
- No PDF processing for existing tenders
- No ML evaluation for existing tenders
- No emails sent for existing tenders

## Setting Up Daily Schedule

### Option 1: AWS EventBridge (Recommended)

Create an EventBridge rule in the AWS Console or via Terraform:

#### Terraform Configuration

Add to `aws_deploy_infrastructure/lambdas.tf`:

```hcl
# EventBridge rule to trigger get_data Lambda daily at 09:00 UTC
resource "aws_cloudwatch_event_rule" "daily_tender_scan" {
  name                = "daily-tender-scan"
  description         = "Trigger tender scanning every day at 09:00 UTC"
  schedule_expression = "cron(0 9 * * ? *)"
}

resource "aws_cloudwatch_event_target" "daily_tender_scan_target" {
  rule      = aws_cloudwatch_event_rule.daily_tender_scan.name
  target_id = "get-data-lambda"
  arn       = aws_lambda_function.get_data.arn
  
  input = jsonencode({
    max_pages = 10
    test_mode = false
    start_page = 1
  })
}

# Permission for EventBridge to invoke Lambda
resource "aws_lambda_permission" "allow_eventbridge_get_data" {
  statement_id  = "AllowExecutionFromEventBridge"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.get_data.function_name
  principal     = "events.amazonaws.com"
  source_arn    = aws_cloudwatch_event_rule.daily_tender_scan.arn
}
```

### Option 2: Manual Setup via AWS Console

1. Go to **AWS Console** → **EventBridge** → **Rules**
2. Click **Create rule**
3. Configure:
   - **Name:** `daily-tender-scan`
   - **Description:** `Trigger tender scanning daily at 09:00 UTC`
   - **Event bus:** `default`
   - **Rule type:** `Schedule`
4. **Schedule pattern:**
   - Pattern type: `Cron expression`
   - Expression: `cron(0 9 * * ? *)`
   - Timezone: `UTC`
5. **Target:**
   - Target type: `AWS service`
   - Select target: `Lambda function`
   - Function: `get_data`
   - Configure input:
     ```json
     {
       "max_pages": 10,
       "test_mode": false,
       "start_page": 1
     }
     ```
6. Click **Create rule**

## Cron Expression Explained

```
cron(0 9 * * ? *)
     │ │ │ │ │ │
     │ │ │ │ │ └─── Year (not specified)
     │ │ │ │ └───── Day of week (?)
     │ │ │ └─────── Month (every month)
     │ │ └───────── Day (every day)
     │ └─────────── Hour (09:00)
     └───────────── Minute (00)
```

- **09:00 UTC** = 09:00 AM London (winter) / 10:00 AM London (summer)
- Runs **every day** including weekends
- Adjust hour if you want different time

### Other Time Examples

```bash
# 07:00 UTC (7 AM)
cron(0 7 * * ? *)

# 12:00 UTC (12 PM)
cron(0 12 * * ? *)

# Every 6 hours
cron(0 0/6 * * ? *)

# Weekdays only at 09:00
cron(0 9 ? * MON-FRI *)
```

## Testing the Setup

### 1. Manual Test via AWS Console

1. Go to **Lambda** → `get_data`
2. Click **Test**
3. Create test event:
   ```json
   {
     "max_pages": 2,
     "test_mode": false,
     "start_page": 1
   }
   ```
4. Click **Test**
5. Check logs in **CloudWatch**

### 2. Check Database After Run

```sql
-- See recently added tenders
SELECT resource_id, title, created_at, notification_sent
FROM tender_records
ORDER BY created_at DESC
LIMIT 10;

-- Count new tenders from today
SELECT COUNT(*)
FROM tender_records
WHERE DATE(created_at) = CURRENT_DATE;

-- Check notification status
SELECT 
  COUNT(*) as total,
  SUM(CASE WHEN notification_sent THEN 1 ELSE 0 END) as notified,
  SUM(CASE WHEN notification_sent THEN 0 ELSE 1 END) as pending
FROM tender_records;
```

### 3. Monitor EventBridge Rule

1. Go to **EventBridge** → **Rules**
2. Select your rule
3. Click **Metrics** tab
4. View:
   - Invocations
   - Failed invocations
   - Trigger count

## Configuration Parameters

### Lambda Input Parameters

```json
{
  "max_pages": 10,        // Number of pages to scan (1 page ≈ 20 tenders)
  "test_mode": false,     // true = don't save to DB, return sample data
  "start_page": 1         // Which page to start from
}
```

### Recommended Settings

**Production (Daily Scan):**
```json
{
  "max_pages": 10,
  "test_mode": false,
  "start_page": 1
}
```

**Testing:**
```json
{
  "max_pages": 1,
  "test_mode": true,
  "start_page": 1
}
```

**Recovery (if system was down):**
```json
{
  "max_pages": 20,
  "test_mode": false,
  "start_page": 1
}
```

## Monitoring & Alerts

### CloudWatch Metrics to Monitor

1. **Lambda Duration:**
   - `get_data` should complete in < 5 minutes
   - Alert if > 10 minutes

2. **Lambda Errors:**
   - Monitor failed invocations
   - Alert on any errors

3. **Database Connection:**
   - Monitor connection pool usage
   - Alert if connections exhausted

4. **SQS Queue Depth:**
   - Monitor queue buildup
   - Alert if messages backing up

### Recommended CloudWatch Alarms

```hcl
# Lambda error alarm
resource "aws_cloudwatch_metric_alarm" "get_data_errors" {
  alarm_name          = "get-data-lambda-errors"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = "1"
  metric_name         = "Errors"
  namespace           = "AWS/Lambda"
  period              = "300"
  statistic           = "Sum"
  threshold           = "1"
  alarm_description   = "Alert when get_data Lambda has errors"
  
  dimensions = {
    FunctionName = aws_lambda_function.get_data.function_name
  }
}
```

## Troubleshooting

### Issue: No new tenders found

**Possible causes:**
1. All tenders already in database (expected on second run)
2. etenders.gov.ie website structure changed
3. Network connectivity issues

**Debug:**
```sql
-- Check last tender added
SELECT MAX(created_at) FROM tender_records;

-- Check if we have recent tenders
SELECT COUNT(*) FROM tender_records 
WHERE created_at > NOW() - INTERVAL '7 days';
```

### Issue: Duplicate emails being sent

**Check notification status:**
```sql
SELECT resource_id, title, notification_sent, notification_sent_at
FROM tender_records
WHERE notification_sent = TRUE
ORDER BY notification_sent_at DESC
LIMIT 20;
```

**Verify columns exist:**
```sql
SELECT column_name, data_type 
FROM information_schema.columns 
WHERE table_name = 'tender_records' 
  AND column_name IN ('notification_sent', 'notification_sent_at');
```

### Issue: Lambda timeout

**Increase timeout:**
```hcl
resource "aws_lambda_function" "get_data" {
  # ... other config ...
  timeout = 900  # Increase from default (15 minutes max)
}
```

## Cost Estimation

### Daily Run Costs (approximate)

- **Lambda execution:** ~$0.01/day
- **Database queries:** ~$0.001/day
- **S3 reads (codes.txt):** ~$0.0001/day
- **SQS messages:** ~$0.001/day
- **SES emails:** ~$0.10/day (assuming 1 notification)

**Total:** ~$0.11/day = **~$3.30/month**

## Next Steps

After setting up the daily schedule:

1. ✅ **Deploy the Terraform changes**
2. ✅ **Monitor first few runs**
3. ✅ **Verify notifications are received**
4. ✅ **Check database for duplicates**
5. ✅ **Set up CloudWatch alarms**
6. ✅ **Document any custom rules or filters**

## Manual Invocation

If you need to trigger a scan manually:

```bash
# Via AWS CLI
aws lambda invoke \
  --function-name get_data \
  --payload '{"max_pages":10,"test_mode":false,"start_page":1}' \
  response.json
```

## Summary

✅ **Deduplication:** Handled at DB level by checking `resource_id`
✅ **No duplicate emails:** Tracked via `notification_sent` flag
✅ **10 pages:** Ensures we don't miss new tenders
✅ **Daily schedule:** 09:00 UTC via EventBridge
✅ **Idempotent:** Safe to run multiple times
✅ **Monitored:** CloudWatch metrics and logs
✅ **Cost-effective:** ~$3-4/month

The system is designed to be reliable, efficient, and prevent any duplicate notifications!