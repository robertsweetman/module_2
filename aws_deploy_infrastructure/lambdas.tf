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
      LAMBDA_BUCKET = aws_s3_bucket.lambda_bucket.id
      PDF_PROCESSING_QUEUE_URL = aws_sqs_queue.pdf_processing_queue.url
    }
  }

  timeout = 120
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
      PDF_PROCESSING_QUEUE_URL = aws_sqs_queue.pdf_processing_queue.url
    }
  }

  timeout = 900
  memory_size = 1024
}

resource "aws_lambda_function" "get_data" {
  function_name = "get_data"
  handler       = "bootstrap"
  runtime       = "provided.al2"
  role          = aws_iam_role.lambda_role.arn
  
  s3_bucket     = aws_s3_bucket.lambda_bucket.id
  s3_key        = "get_data.zip"

  depends_on = [
    aws_s3_bucket.lambda_bucket,
    aws_s3_object.codes_file
  ]
  lifecycle {
    ignore_changes = [source_code_hash]
  }
  
  environment {
    variables = {
      RUST_BACKTRACE  = "1"
      DATABASE_URL = "postgres://${var.db_admin_name}:${var.db_admin_pwd}@${aws_db_instance.postgres.endpoint}/${var.db_name}"
      LAMBDA_BUCKET_NAME = aws_s3_bucket.lambda_bucket.id
    }
  }

  timeout = 900
  memory_size = 1024
}