# Output the database endpoint
output "db_endpoint" {
  description = "postgres database endpoint"
  value       = aws_db_instance.postgres.endpoint
}

output "db_name" {
  description = "postgres database name"
  value       = aws_db_instance.postgres.db_name
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
  value       = aws_sns_topic.ml_predictions.arn
  description = "ARN of the ML predictions SNS topic"
}

output "sqs_queue_url" {
  value       = aws_sqs_queue.sns_queue.url
  description = "URL of the SNS notification queue"
}

# Bastion host outputs
output "bastion_instance_id" {
  description = "Instance ID of the bastion host"
  value       = aws_instance.bastion.id
}

output "bastion_connect_command" {
  description = "AWS CLI command to connect to bastion host via SSM"
  value       = "aws ssm start-session --target ${aws_instance.bastion.id}"
}

output "database_tunnel_instructions" {
  description = "Instructions for setting up a secure tunnel to the database"
  value       = <<-EOF
To access the PostgreSQL database securely:

1. Connect to bastion host:
   aws ssm start-session --target ${aws_instance.bastion.id}

2. Once connected, use the pre-installed connection script:
   ./connect-db.sh

3. Or connect directly with psql:
   psql -h ${aws_db_instance.postgres.endpoint} -p 5432 -U ${var.db_admin_name} -d ${var.db_name}

4. For port forwarding (access from your local machine):
   aws ssm start-session --target ${aws_instance.bastion.id} \
     --document-name AWS-StartPortForwardingSessionToRemoteHost \
     --parameters host="${aws_db_instance.postgres.endpoint}",portNumber="5432",localPortNumber="5432"

   Then connect locally: psql -h localhost -p 5432 -U ${var.db_admin_name} -d ${var.db_name}
EOF
}