# Secrets and deployment steps

Required:
* AWS_ACCESS_KEY_ID
* AWS_REGION
* AWS_SECRET_ACCESS_KEY

## Create the S3 bucket for backend state first

1. Run terraform manually from the local machine with s3_backend_bootstrap.tf 
2. Then update the providers.tf file in the aws_Deploy_infrastructure folder with the s3 bucket created by the bootstrap stage
3. Then run the terraform_plan or terraform_plan_and_apply.yml to deploy everything else
4. 