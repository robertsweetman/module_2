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
    region = var.aws_region
  }
}

provider "aws" {
  region = var.aws_region
}