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

variable "notification_emails_str" {
  description = "Comma-separated string of notification email addresses (from GitHub secrets)"
  type        = string
  default     = ""
}

variable "from_email" {
  description = "Email address to use as sender for notifications"
  type        = string
  default     = "etenders-noreply@robertsweetman.com"
}