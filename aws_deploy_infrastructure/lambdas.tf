resource "aws_lambda_function" "pdf_processing" {
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