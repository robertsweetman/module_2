# This secret stores the connection details for the eTenders PostgreSQL instance so that
# application code (Python notebooks, Lambdas, etc.) can retrieve them securely at runtime.

resource "aws_secretsmanager_secret" "db_credentials" {
  name = var.db_credentials_secret_name
  description = "Connection credentials for the eTenders PostgreSQL database"
  recovery_window_in_days = 0 # allow immediate deletion if necessary
}

