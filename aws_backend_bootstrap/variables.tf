variable "aws_region" {
  type    = string
  default = "eu-west-2"
}

# TODO: look this up after it's been created instead rather than hardcoding?
variable "tf_state_bucket_name" {
  type    = string
  default = "tfstate"
}
