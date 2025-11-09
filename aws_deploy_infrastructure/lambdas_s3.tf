resource "aws_s3_bucket" "lambda_bucket" {
  bucket        = "module2-lambda-deployments"
  force_destroy = true
}

# Block all public access
resource "aws_s3_bucket_public_access_block" "lambda_bucket_access" {
  bucket = aws_s3_bucket.lambda_bucket.id

  block_public_acls       = true
  block_public_policy     = true
  ignore_public_acls      = true
  restrict_public_buckets = true
}

# Enable server-side encryption
resource "aws_s3_bucket_server_side_encryption_configuration" "lambda_bucket_encryption" {
  bucket = aws_s3_bucket.lambda_bucket.id

  rule {
    apply_server_side_encryption_by_default {
      sse_algorithm = "AES256"
    }
  }
}

# Restrict bucket policy to only allow access from your AWS account
resource "aws_s3_bucket_policy" "lambda_bucket_policy" {
  bucket = aws_s3_bucket.lambda_bucket.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Sid       = "DenyExternalAccess"
        Effect    = "Deny"
        Principal = "*"
        Action    = "s3:*"
        Resource = [
          "${aws_s3_bucket.lambda_bucket.arn}",
          "${aws_s3_bucket.lambda_bucket.arn}/*"
        ]
        Condition = {
          StringNotEquals = {
            "aws:PrincipalAccount" : "${data.aws_caller_identity.current.account_id}"
          }
        }
      }
    ]
  })
}

# Add AWS caller identity data source
data "aws_caller_identity" "current" {}

# Upload codes.txt to S3
resource "aws_s3_object" "codes_file" {
  bucket = aws_s3_bucket.lambda_bucket.id
  key    = "codes.txt"
  source = "${path.module}/../codes.txt"
  etag   = filemd5("${path.module}/../codes.txt")
}

# Note: The get_data Lambda function uses the shared lambda_role which already has s3:* permissions
# No additional IAM configuration needed since the shared role covers all S3 access


