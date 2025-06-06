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