# SES Email Identity Verification
resource "aws_ses_email_identity" "sender_email" {
  email = var.from_email
}

# SES Configuration Set (optional - for tracking)
resource "aws_ses_configuration_set" "etenders_notifications" {
  name = "etenders-notifications"

  delivery_options {
    tls_policy = "Require"
  }
}

# Output the verification status
output "ses_email_verification_status" {
  value = aws_ses_email_identity.sender_email.arn
  description = "ARN of the verified SES email identity"
}

# Note: After terraform apply, you still need to click the verification link
# that AWS sends to etenders-noreply@robertsweetman.com
