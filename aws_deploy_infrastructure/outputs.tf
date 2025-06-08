# Output the database endpoint
output "db_endpoint" {
  value = aws_db_instance.postgres.endpoint
}

output "db_name" {
  value = aws_db_instance.postgres.db_name
}

output "pdf_processing_step_function_arn" {
  description = "ARN of the PDF processing Step Function workflow"
  value       = aws_sfn_state_machine.pdf_processing_workflow.arn
}

output "lambda_bucket_name" {
  description = "Name of the S3 bucket for Lambda deployment packages"
  value       = aws_s3_bucket.lambda_bucket.bucket
}