# Check if the deployment packages exist in S3
data "aws_s3_object" "pdf_processing_zip" {
  bucket = aws_s3_bucket.lambda_bucket.bucket
  key    = "pdf_processing.zip"
}

data "aws_s3_object" "postgres_dataload_zip" {
  bucket = aws_s3_bucket.lambda_bucket.bucket
  key    = "postgres_dataload.zip"
}

locals {
  pdf_processing_exists = try(data.aws_s3_object.pdf_processing_zip.content_length, 0) > 0
  postgres_dataload_exists = try(data.aws_s3_object.postgres_dataload_zip.content_length, 0) > 0
}

resource "aws_lambda_function" "pdf_processing" {
  count         = local.pdf_processing_exists ? 1 : 0

  function_name = "pdf-processing"
  handler       = "bootstrap"
  runtime       = "provided.al2"
  role          = aws_iam_role.lambda_role.arn
  
  s3_bucket     = aws_s3_bucket.lambda_bucket.bucket
  s3_key        = "pdf_processing.zip"
  
  environment {
    variables = {
      DATABASE_URL = "postgres://${var.db_admin_name}:${var.db_admin_pwd}@${aws_db_instance.postgres.endpoint}/${var.db_name}"
    }
  }
  
  timeout = 60
  memory_size = 512
}

resource "aws_lambda_function" "postgres_dataload" {
  count         = local.postgres_dataload_exists ? 1 : 0

  function_name = "postgres-dataload"
  handler       = "bootstrap"
  runtime       = "provided.al2"
  role          = aws_iam_role.lambda_role.arn
  
  s3_bucket     = aws_s3_bucket.lambda_bucket.bucket
  s3_key        = "postgres_dataload.zip"
  
  environment {
    variables = {
      DATABASE_URL = "postgres://${var.db_admin_name}:${var.db_admin_pwd}@${aws_db_instance.postgres.endpoint}/${var.db_name}"
      PDF_PROCESSING_STEP_FUNCTION_ARN = aws_sfn_state_machine.pdf_processing_workflow.arn
    }
  }
  
  timeout = 60
  memory_size = 512
}