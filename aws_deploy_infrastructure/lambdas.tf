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
  
  s3_bucket     = aws_s3_bucket.lambda_bucket.id
  s3_key        = "pdf_processing.zip"

  depends_on = [aws_s3_bucket.lambda_bucket]
  lifecycle {
    ignore_changes = [source_code_hash]
  }
  
  environment {
    variables = {
      RUST_BACKTRACE = "full"
      DATABASE_URL = "postgres://${var.db_admin_name}:${var.db_admin_pwd}@${aws_db_instance.postgres.endpoint}/${var.db_name}"
    }
  }

  timeout = 900
  memory_size = 1024
}

resource "aws_lambda_function" "postgres_dataload" {
  function_name = "postgres_dataload"
  handler       = "bootstrap"
  runtime       = "provided.al2"
  role          = aws_iam_role.lambda_role.arn
  
  s3_bucket     = aws_s3_bucket.lambda_bucket.id
  s3_key        = "postgres_dataload.zip"

  depends_on = [aws_s3_bucket.lambda_bucket]
  lifecycle {
    ignore_changes = [source_code_hash]
  }
  
  environment {
    variables = {
      RUST_BACKTRACE  = "1"
      DATABASE_URL = "postgres://${var.db_admin_name}:${var.db_admin_pwd}@${aws_db_instance.postgres.endpoint}/${var.db_name}"
      PDF_PROCESSING_STEP_FUNCTION_ARN = aws_sfn_state_machine.pdf_processing_workflow.arn
    }
  }

  timeout = 900
  memory_size = 1024
}