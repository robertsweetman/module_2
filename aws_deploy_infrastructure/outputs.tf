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