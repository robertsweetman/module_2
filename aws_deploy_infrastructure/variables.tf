variable "aws_region" {
    type = string
    default = "eu-west-2"
}

variable "db_name" {
    type = string
    default = "etenders"
}

// secret from Github
variable "db_admin_name" {
    type = string
    description = "Database administrator username"
    # no default - will be provided by github actions
}

variable "db_admin_pwd" {
    type = string
    description = "Database administrator password"
    sensitive = true
    # no default - provided by github actions
}

variable "db_credentials_secret_name" {
  description = "Name of the Secrets Manager secret that will hold the PostgreSQL credentials"
  type        = string
  default     = "etenders_rds_credentials"
}

variable "db_credentials_initial_secret_string" {
  description = "Initial JSON payload for the DB credentials secret. Override via TF_VAR_... or terraform.tfvars."
  type        = string
  default     = <<EOF
{
  "host": "your-rds-endpoint.amazonaws.com",
  "port": 5432,
  "username": "username",
  "password": "password",
  "database": "etenders"
}
EOF
}