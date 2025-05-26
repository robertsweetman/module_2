resource "random_string" "tf_state_bucket_suffix" {
  length  = 8
  upper   = false
  special = false
}

# S3 bucket for storing terraform state
resource "aws_s3_bucket" "tf_state_bucket" {
  bucket = "${var.tf_state_bucket_name}-${random_string.tf_state_bucket_suffix.result}"

  lifecycle {
    prevent_destroy = true
  }
}

resource "aws_s3_bucket_server_side_encryption_configuration" "tf_state_bucket_encryption" {
  bucket = aws_s3_bucket.tf_state_bucket.id

  rule {
    apply_server_side_encryption_by_default {
      sse_algorithm = "aws:kms"
    }
  }
}

resource "aws_s3_bucket_public_access_block" "tf_state_bucket_public_access_block" {
  bucket = aws_s3_bucket.tf_state_bucket.id

  block_public_acls       = true
  block_public_policy     = true
  ignore_public_acls      = true
  restrict_public_buckets = true
}
