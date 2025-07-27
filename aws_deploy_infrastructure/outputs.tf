# Output the database endpoint
output "db_endpoint" {
  description = "postgres database endpoint"
  value = aws_db_instance.postgres.endpoint
}

output "db_name" {
  description = "postgres database name"
  value = aws_db_instance.postgres.db_name
}

output "lambda_bucket_name" {
  description = "Name of the S3 bucket for Lambda deployment packages"
  value       = aws_s3_bucket.lambda_bucket.bucket
}

output "pdf_processing_queue_url" {
  description = "URL of the PDF processing queue"
  value       = aws_sqs_queue.pdf_processing_queue.url
}

output "pdf_processing_dlq_url" {
  value = aws_sqs_queue.pdf_processing_dlq.url
}

output "db_credentials_secret_arn" {
  description = "ARN of the Secrets Manager secret that stores DB credentials"
  value       = aws_secretsmanager_secret.db_credentials.arn
} 

# Output the topic ARN for use in other resources
output "sns_topic_arn" {
  value = aws_sns_topic.ml_predictions.arn
  description = "ARN of the ML predictions SNS topic"
}

output "sqs_queue_url" {
  value = aws_sqs_queue.sns_queue.url
  description = "URL of the SNS notification queue"
}

output "ai_summary_queue_url" {
  value = aws_sqs_queue.ai_summary_queue.url
  description = "URL of the AI summary queue"
}

output "ml_prediction_queue_url" {
  value = aws_sqs_queue.ml_prediction_queue.url
  description = "URL of the ML prediction queue"
}

# Dead Letter Queue URLs for monitoring
output "sns_dlq_url" {
  value = aws_sqs_queue.sns_dlq.url
  description = "URL of the SNS notification dead letter queue"
}

output "ai_summary_dlq_url" {
  value = aws_sqs_queue.ai_summary_dlq.url
  description = "URL of the AI summary dead letter queue"
}

output "ml_prediction_dlq_url" {
  value = aws_sqs_queue.ml_prediction_dlq.url
  description = "URL of the ML prediction dead letter queue"
}