terraform {
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "= 6.0.0-beta2"
    }
  }

  # backend state is held in S3
  backend "s3" {
    bucket = "tfstate-a3zfrygj" # IMPORTANT: update AFTER aws_backend_bootstrap has been run
    key    = "terraform.tfstate"
    region = "eu-west-2"
  }
}

provider "aws" {
  region = "eu-west-2"
}