# Output the database endpoint
output "db_endpoint" {
  description = "postgres database endpoint"
  value       = data.aws_db_instance.postgres.endpoint
}

output "db_name" {
  description = "postgres database name"
  value       = data.aws_db_instance.postgres.db_name
}

output "lambda_bucket_name" {
  description = "S3 bucket name for Lambda functions"
  value       = aws_s3_bucket.lambda_bucket.id
}

output "lambda_role_arn" {
  description = "IAM role ARN for Lambda functions"
  value       = aws_iam_role.lambda_role.arn
}

output "sqs_queue_urls" {
  description = "URLs of all SQS queues"
  value = {
    pdf_processing    = aws_sqs_queue.pdf_processing_queue.url
    ml_prediction     = aws_sqs_queue.ml_prediction_queue.url
    ai_summary        = aws_sqs_queue.ai_summary_queue.url
    sns_notification  = aws_sqs_queue.sns_queue.url
    tender_processing = aws_sqs_queue.tender_processing_queue.url
  }
}

output "lambda_function_names" {
  description = "Names of all Lambda functions"
  value = {
    pdf_processing    = aws_lambda_function.pdf_processing.function_name
    postgres_dataload = aws_lambda_function.postgres_dataload.function_name
    get_data          = aws_lambda_function.get_data.function_name
    ml_bid_predictor  = aws_lambda_function.ml_bid_predictor.function_name
    ai_summary        = aws_lambda_function.ai_summary.function_name
    sns_notification  = aws_lambda_function.sns_notification.function_name
    etenders_scraper  = aws_lambda_function.etenders_scraper.function_name
  }
}

output "bastion_instance_id" {
  description = "Instance ID of the bastion host for SSM access"
  value       = aws_instance.bastion.id
}

output "sns_topic_arn" {
  description = "ARN of the ML predictions SNS topic"
  value       = aws_sns_topic.ml_predictions.arn
}
