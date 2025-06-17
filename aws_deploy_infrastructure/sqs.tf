# SQS Queue for PDF processing
resource "aws_sqs_queue" "pdf_processing_queue" {
  name                      = "pdf-processing-queue"
  visibility_timeout_seconds = 300  # 5 minutes (longer than your Lambda timeout)
  message_retention_seconds = 1209600  # 14 days
  receive_wait_time_seconds = 20  # Long polling
  
  redrive_policy = jsonencode({
    deadLetterTargetArn = aws_sqs_queue.pdf_processing_dlq.arn
    maxReceiveCount     = 5  # Retry 3 times before moving to DLQ
  })

  tags = {
    Name = "PDF Processing Queue"
  }
}

# Dead Letter Queue for failed messages
resource "aws_sqs_queue" "pdf_processing_dlq" {
  name                      = "pdf-processing-dlq"
  message_retention_seconds = 1209600  # 14 days

  tags = {
    Name = "PDF Processing Dead Letter Queue"
  }
}

# Lambda trigger from SQS
resource "aws_lambda_event_source_mapping" "pdf_processing_sqs_trigger" {
  event_source_arn = aws_sqs_queue.pdf_processing_queue.arn
  function_name    = aws_lambda_function.pdf_processing.function_name
  
  batch_size       = 1  # Process one PDF at a time
  maximum_batching_window_in_seconds = 0  # Disable extra buffering; one message per invoke
  
  scaling_config {
    maximum_concurrency = 200  # Control concurrency here instead of reserved concurrency
  }
}

# IAM permissions for Lambda to read from SQS
resource "aws_iam_policy" "lambda_sqs_policy" {
  name = "lambda-sqs-pdf-processing-policy"

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Action = [
          "sqs:ReceiveMessage",
          "sqs:DeleteMessage",
          "sqs:GetQueueAttributes"
        ]
        Resource = [
          aws_sqs_queue.pdf_processing_queue.arn,
          aws_sqs_queue.pdf_processing_dlq.arn
        ]
      }
    ]
  })
}

# Attach policy to your existing Lambda role
resource "aws_iam_role_policy_attachment" "lambda_sqs_policy_attachment" {
  policy_arn = aws_iam_policy.lambda_sqs_policy.arn
  role       = aws_iam_role.lambda_role.name
}

# IAM permissions for postgres_dataload Lambda to send to SQS
resource "aws_iam_policy" "postgres_dataload_sqs_policy" {
  name = "postgres-dataload-sqs-policy"

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Action = [
          "sqs:SendMessage",
          "sqs:GetQueueUrl"
        ]
        Resource = aws_sqs_queue.pdf_processing_queue.arn
      }
    ]
  })
}

# Attach to postgres_dataload Lambda role
resource "aws_iam_role_policy_attachment" "postgres_dataload_sqs_policy_attachment" {
  policy_arn = aws_iam_policy.postgres_dataload_sqs_policy.arn
  role       = aws_iam_role.lambda_role.name
}

