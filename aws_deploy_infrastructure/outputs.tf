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
  value       = aws_s3_bucket.lambda_bucket.s3_bucket
}

output "pdf_processing_queue_url" {
  description = "URL of the PDF processing queue"
  value       = aws_sqs_queue.pdf_processing_queue.url
}