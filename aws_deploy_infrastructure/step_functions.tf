resource "aws_sfn_state_machine" "pdf_processing_workflow" {
  name     = "pdf-processing-workflow"
  role_arn = aws_iam_role.step_functions_role.arn

  definition = <<EOF
{
  "Comment": "PDF Processing Workflow",
  "StartAt": "ProcessTenderPDFs",
  "States": {
    "ProcessTenderPDFs": {
      "Type": "Map",
      "ItemsPath": "$.records",
      "Iterator": {
        "StartAt": "ProcessSinglePDF",
        "States": {
          "ProcessSinglePDF": {
            "Type": "Task",
            "Resource": "${aws_lambda_function.pdf_processing.arn}",
            "Retry": [
              {
                "ErrorEquals": ["States.ALL"],
                "IntervalSeconds": 2,
                "MaxAttempts": 2,
                "BackoffRate": 2.0
              }
            ],
            "End": true
          }
        }
      },
      "End": true
    }
  }
}
EOF
}

resource "aws_iam_role" "step_functions_role" {
  name = "step_functions_role"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Principal = {
          Service = "states.amazonaws.com"
        }
        Action = "sts:AssumeRole"
      }
    ]
  })
}

resource "aws_iam_role_policy" "step_functions_policy" {
  name = "step_functions_policy"
  role = aws_iam_role.step_functions_role.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Action = [
          "lambda:InvokeFunction"
        ]
        Resource = [
          aws_lambda_function.pdf_processing.arn
        ]
      }
    ]
  })
}

resource "aws_lambda_function" "pdf_processing" {
  function_name = "pdf-processing"
  handler       = "bootstrap"
  runtime       = "provided.al2"
  role          = aws_iam_role.lambda_role.arn
  
  filename      = "../crates/pdf_processing/target/lambda/pdf-processing/bootstrap.zip"
  
  environment {
    variables = {
      DATABASE_URL = "postgres://${var.db_admin_name}:${var.db_admin_pwd}@${aws_db_instance.postgres.endpoint}/${var.db_name}"
    }
  }
  
  timeout = 60  # PDFs can take time to process
  memory_size = 512
}
