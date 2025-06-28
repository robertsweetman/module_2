# This secret stores the connection details for the eTenders PostgreSQL instance so that
# application code (Python notebooks, Lambdas, etc.) can retrieve them securely at runtime.

resource "aws_secretsmanager_secret" "db_credentials" {
  name = var.db_credentials_secret_name
  description = "Connection credentials for the eTenders PostgreSQL database"
  recovery_window_in_days = 0 # allow immediate deletion if necessary
}

# Optionally seed the secret with an initial JSON payload.  Override via the AWS console later.
resource "aws_secretsmanager_secret_version" "db_credentials_version" {
  secret_id     = aws_secretsmanager_secret.db_credentials.id
  secret_string = var.db_credentials_initial_secret_string
}
