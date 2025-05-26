# Secrets and deployment steps

Required in LOCAL ENVIRONMENT:

* AWS_ACCESS_KEY_ID
* AWS_REGION
* AWS_SECRET_ACCESS_KEY

Run terraform init/plan/apply in this folder FIRST

Update the providers.tf with the backend bucket name

Then cd ..\aws_deploy_infrastructure and init/plan/apply from there.

This ensures that the state is held in AWS always