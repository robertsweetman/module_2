# Get default VPC subnets
data "aws_subnets" "default" {
  filter {
    name   = "vpc-id"
    values = [data.aws_vpc.default.id]
  }
}

resource "aws_lambda_function" "pdf_processing" {
  function_name = "pdf_processing"
  handler       = "bootstrap"
  runtime       = "provided.al2"
  role          = aws_iam_role.lambda_role.arn

  s3_bucket = aws_s3_bucket.lambda_bucket.id
  s3_key    = "pdf_processing.zip"

  depends_on = [aws_s3_bucket.lambda_bucket]
  lifecycle {
    ignore_changes = [source_code_hash]
  }

  # VPC Configuration
  vpc_config {
    subnet_ids         = tolist(data.aws_subnets.default.ids)
    security_group_ids = [aws_security_group.lambda_sg.id]
  }

  environment {
    variables = {
      RUST_BACKTRACE           = "full"
      DATABASE_URL             = "postgres://${var.db_admin_name}:${var.db_admin_pwd}@${aws_db_instance.postgres.endpoint}/${var.db_name}"
      LAMBDA_BUCKET            = aws_s3_bucket.lambda_bucket.id
      PDF_PROCESSING_QUEUE_URL = aws_sqs_queue.pdf_processing_queue.url
      ML_PREDICTION_QUEUE_URL  = aws_sqs_queue.ml_prediction_queue.url
    }
  }

  timeout     = 120
  memory_size = 1024
}

resource "aws_lambda_function" "postgres_dataload" {
  function_name = "postgres_dataload"
  handler       = "bootstrap"
  runtime       = "provided.al2"
  role          = aws_iam_role.lambda_role.arn

  s3_bucket = aws_s3_bucket.lambda_bucket.id
  s3_key    = "postgres_dataload.zip"

  depends_on = [aws_s3_bucket.lambda_bucket]
  lifecycle {
    ignore_changes = [source_code_hash]
  }

  # VPC Configuration
  vpc_config {
    subnet_ids         = tolist(data.aws_subnets.default.ids)
    security_group_ids = [aws_security_group.lambda_sg.id]
  }

  environment {
    variables = {
      RUST_BACKTRACE           = "1"
      DATABASE_URL             = "postgres://${var.db_admin_name}:${var.db_admin_pwd}@${aws_db_instance.postgres.endpoint}/${var.db_name}"
      PDF_PROCESSING_QUEUE_URL = aws_sqs_queue.pdf_processing_queue.url
      ML_PREDICTION_QUEUE_URL  = aws_sqs_queue.ml_prediction_queue.url
      AI_SUMMARY_QUEUE_URL     = aws_sqs_queue.ai_summary_queue.url
    }
  }

  timeout     = 900
  memory_size = 1024
}

resource "aws_lambda_function" "get_data" {
  function_name = "get_data"
  handler       = "bootstrap"
  runtime       = "provided.al2"
  role          = aws_iam_role.lambda_role.arn

  s3_bucket = aws_s3_bucket.lambda_bucket.id
  s3_key    = "get_data.zip"

  depends_on = [
    aws_s3_bucket.lambda_bucket,
    aws_s3_object.codes_file
  ]
  lifecycle {
    ignore_changes = [source_code_hash]
  }

  # VPC Configuration
  vpc_config {
    subnet_ids         = tolist(data.aws_subnets.default.ids)
    security_group_ids = [aws_security_group.lambda_sg.id]
  }

  environment {
    variables = {
      RUST_BACKTRACE     = "1"
      DATABASE_URL       = "postgres://${var.db_admin_name}:${var.db_admin_pwd}@${aws_db_instance.postgres.endpoint}/${var.db_name}"
      LAMBDA_BUCKET_NAME = aws_s3_bucket.lambda_bucket.id
    }
  }

  timeout     = 900
  memory_size = 1024
}

resource "aws_lambda_function" "ml_bid_predictor" {
  function_name = "ml_bid_predictor"
  handler       = "bootstrap"
  runtime       = "provided.al2"
  role          = aws_iam_role.lambda_role.arn

  s3_bucket = aws_s3_bucket.lambda_bucket.id
  s3_key    = "ml_bid_predictor.zip"

  depends_on = [aws_s3_bucket.lambda_bucket]
  lifecycle {
    ignore_changes = [source_code_hash]
  }

  # VPC Configuration
  vpc_config {
    subnet_ids         = tolist(data.aws_subnets.default.ids)
    security_group_ids = [aws_security_group.lambda_sg.id]
  }

  environment {
    variables = {
      RUST_BACKTRACE       = "1"
      DATABASE_URL         = "postgres://${var.db_admin_name}:${var.db_admin_pwd}@${aws_db_instance.postgres.endpoint}/${var.db_name}"
      AI_SUMMARY_QUEUE_URL = aws_sqs_queue.ai_summary_queue.url
      SNS_TOPIC_ARN        = aws_sns_topic.ml_predictions.arn
      MODEL_VERSION        = "v1.0"
      PREDICTION_THRESHOLD = "0.5"
      BATCH_SIZE           = "100"
      MAX_PDF_TEXT_LENGTH  = "50000"
      MIN_PDF_TEXT_LENGTH  = "50"
    }
  }

  timeout     = 600  # 10 minutes for ML processing
  memory_size = 1024 # More memory for ML computations
}

resource "aws_lambda_function" "ai_summary" {
  function_name = "ai_summary"
  handler       = "bootstrap"
  runtime       = "provided.al2"
  role          = aws_iam_role.lambda_role.arn

  s3_bucket = aws_s3_bucket.lambda_bucket.id
  s3_key    = "ai_summary.zip"

  depends_on = [aws_s3_bucket.lambda_bucket]
  lifecycle {
    ignore_changes = [source_code_hash]
  }

  # VPC Configuration
  vpc_config {
    subnet_ids         = tolist(data.aws_subnets.default.ids)
    security_group_ids = [aws_security_group.lambda_sg.id]
  }

  environment {
    variables = {
      RUST_BACKTRACE    = "1"
      DATABASE_URL      = "postgres://${var.db_admin_name}:${var.db_admin_pwd}@${aws_db_instance.postgres.endpoint}/${var.db_name}"
      SNS_QUEUE_URL     = aws_sqs_queue.sns_queue.url
      ANTHROPIC_API_KEY = var.anthropic_api_key
    }
  }

  timeout     = 300 # 5 minutes for AI processing
  memory_size = 512 # Moderate memory for AI API calls
}

resource "aws_lambda_function" "sns_notification" {
  function_name = "sns_notification"
  handler       = "bootstrap"
  runtime       = "provided.al2"
  role          = aws_iam_role.lambda_role.arn

  s3_bucket = aws_s3_bucket.lambda_bucket.id
  s3_key    = "sns_notification.zip"

  depends_on = [aws_s3_bucket.lambda_bucket]
  lifecycle {
    ignore_changes = [source_code_hash]
  }

  environment {
    variables = {
      RUST_BACKTRACE      = "1"
      NOTIFICATION_EMAILS = var.notification_emails_str
      FROM_EMAIL          = var.from_email
    }
  }

  timeout     = 60  # 1 minute for email notifications
  memory_size = 256 # Minimal memory for email sending
}

resource "aws_lambda_function" "etenders_scraper" {
  function_name = "etenders_scraper"
  handler       = "bootstrap"
  runtime       = "provided.al2"
  role          = aws_iam_role.lambda_role.arn

  s3_bucket = aws_s3_bucket.lambda_bucket.id
  s3_key    = "etenders_scraper.zip"

  depends_on = [aws_s3_bucket.lambda_bucket]
  lifecycle {
    ignore_changes = [source_code_hash]
  }

  # No VPC Configuration - needs internet access to scrape etenders.gov.ie
  # This Lambda only scrapes and sends to SQS, doesn't need database access

  environment {
    variables = {
      RUST_BACKTRACE              = "1"
      TENDER_PROCESSING_QUEUE_URL = aws_sqs_queue.tender_processing_queue.url
    }
  }

  timeout     = 300 # 5 minutes for scraping
  memory_size = 512
}

# EventBridge rule to trigger etenders_scraper Lambda daily at 11:00 UTC (12:00 UK time)
resource "aws_cloudwatch_event_rule" "daily_tender_scan" {
  name                = "daily-tender-scan"
  description         = "Trigger tender scanning every day at 11:00 UTC (12:00 UK time)"
  schedule_expression = "cron(25 14 * * ? *)"
}

resource "aws_cloudwatch_event_target" "daily_tender_scan_target" {
  rule      = aws_cloudwatch_event_rule.daily_tender_scan.name
  target_id = "etenders-scraper-lambda"
  arn       = aws_lambda_function.etenders_scraper.arn

  input = jsonencode({
    max_pages  = 10
    test_mode  = false
    start_page = 1
  })
}

# Permission for EventBridge to invoke Lambda
resource "aws_lambda_permission" "allow_eventbridge_etenders_scraper" {
  statement_id  = "AllowExecutionFromEventBridge"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.etenders_scraper.function_name
  principal     = "events.amazonaws.com"
  source_arn    = aws_cloudwatch_event_rule.daily_tender_scan.arn
}
