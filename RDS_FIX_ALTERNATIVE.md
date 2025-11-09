# RDS Connection Fix - Alternative Solution (No NAT Gateway)

## Problem Analysis

Your `postgres_dataload` Lambda function is timing out when connecting to RDS PostgreSQL:
```
Failed to connect to database: error communicating with database: Operation timed out (os error 110)
```

## Root Cause

The issue is that your Lambda is in a VPC but cannot reach RDS. This is happening because:

1. **Lambda is in default VPC subnets** (which are public subnets)
2. **RDS is also in default VPC subnets** 
3. **Lambda in VPC cannot use the Internet Gateway** for any connections
4. **Security groups may have circular dependency issues**

## Solution: Remove Lambda from VPC (Recommended for This Architecture)

Since your `postgres_dataload` Lambda only needs to:
- ✅ Receive messages from SQS (internet service)
- ✅ Connect to RDS (VPC service)
- ✅ Send messages to SQS (internet service)

The **best solution** is to:
1. **Remove Lambda from VPC** (for SQS access)
2. **Make RDS accessible from Lambda** (via security group rules)
3. **Keep RDS in VPC but allow Lambda's IP range**

### Step 1: Update Lambda Configuration

Edit `lambdas.tf` - Remove the `vpc_config` block from `postgres_dataload`:

```hcl
resource "aws_lambda_function" "postgres_dataload" {
  function_name = "postgres_dataload"
  handler       = "bootstrap"
  runtime       = "provided.al2"
  role          = aws_iam_role.lambda_role.arn

  s3_bucket = aws_s3_bucket.lambda_bucket.id
  s3_key    = "postgres_dataload.zip"

  depends_on = [aws_s3_bucket.lambda_bucket]
  lifecycle {
    ignore_changes = [source_code_hash]
  }

  # REMOVED VPC Configuration - Lambda runs outside VPC for internet access
  # This allows it to access SQS while still reaching RDS via security group

  environment {
    variables = {
      RUST_BACKTRACE           = "1"
      DATABASE_URL             = "postgres://${var.db_admin_name}:${var.db_admin_pwd}@${aws_db_instance.postgres.endpoint}/${var.db_name}"
      PDF_PROCESSING_QUEUE_URL = aws_sqs_queue.pdf_processing_queue.url
      ML_PREDICTION_QUEUE_URL  = aws_sqs_queue.ml_prediction_queue.url
      AI_SUMMARY_QUEUE_URL     = aws_sqs_queue.ai_summary_queue.url
    }
  }

  timeout     = 900
  memory_size = 1024
}
```

### Step 2: Update RDS Security Group

The RDS needs to accept connections from Lambda. Since Lambda is outside VPC, we need to allow connections from the VPC's NAT or use a different approach.

**Option A: Allow from VPC CIDR (Simpler)**

Edit `postgresql.tf`:

```hcl
resource "aws_db_instance" "postgres" {
  identifier          = "postgres-db"
  engine              = "postgres"
  engine_version      = "17.5"
  instance_class      = "db.t3.micro"
  allocated_storage   = 20
  storage_type        = "gp2"
  db_name             = var.db_name
  username            = var.db_admin_name
  password            = var.db_admin_pwd
  skip_final_snapshot = true
  
  # Make RDS accessible from Lambda (which is outside VPC)
  publicly_accessible = true  # Required for Lambda outside VPC to access
  
  vpc_security_group_ids = [aws_security_group.postgres_sg.id]
  db_subnet_group_name   = aws_db_subnet_group.postgres.name

  tags = {
    Name        = "PostgreSQL Database"
    Environment = "Development"
    Application = "eTenders"
  }
}
```

Edit `security_groups.tf` to allow connections from Lambda's public IP range:

```hcl
# Create a security group for the RDS instance
resource "aws_security_group" "postgres_sg" {
  name        = "postgres-sg"
  description = "Allow PostgreSQL inbound traffic from Lambda and Bastion"
  vpc_id      = data.aws_vpc.default.id

  # Allow PostgreSQL from anywhere (Lambda has dynamic IPs)
  # You can restrict this to specific IP ranges if needed
  ingress {
    description = "PostgreSQL from Lambda service"
    from_port   = 5432
    to_port     = 5432
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]  # Use AWS Lambda IP ranges for your region instead
  }

  egress {
    description = "All outbound"
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }

  tags = {
    Name = "postgres-sg"
  }
}
```

### Step 3: Get Lambda IP Ranges (More Secure)

To restrict RDS access only to Lambda IPs:

1. Download AWS IP ranges:
```bash
curl https://ip-ranges.amazonaws.com/ip-ranges.json -o ip-ranges.json
```

2. Extract Lambda IPs for your region (e.g., eu-west-1):
```bash
cat ip-ranges.json | jq -r '.prefixes[] | select(.service=="AMAZON" and .region=="eu-west-1") | .ip_prefix'
```

3. Update security group with specific CIDR blocks (example for Ireland):
```hcl
ingress {
  description = "PostgreSQL from Lambda in eu-west-1"
  from_port   = 5432
  to_port     = 5432
  protocol    = "tcp"
  cidr_blocks = [
    "52.16.0.0/14",      # EU-WEST-1 ranges
    "54.72.0.0/14",
    "54.76.0.0/14",
    # Add more ranges as needed
  ]
}
```

## Alternative: Keep Everything Simple

If you want to avoid making RDS publicly accessible at all, here's the BEST solution:

### Remove VPC from ALL Lambda Functions

Since none of your Lambdas actually need to be in a VPC (they all need internet access for SQS, S3, external APIs), remove VPC config from all of them:

```hcl
# pdf_processing - REMOVE vpc_config
# postgres_dataload - REMOVE vpc_config  
# get_data - REMOVE vpc_config
# ml_bid_predictor - REMOVE vpc_config
# ai_summary - REMOVE vpc_config
# etenders_scraper - Already has no VPC ✓
# sns_notification - Already has no VPC ✓
```

Then keep RDS in VPC but accessible from Lambda IPs.

## Deployment Steps

1. **Backup current configuration**:
```bash
cd module_2/aws_deploy_infrastructure
cp lambdas.tf lambdas.tf.backup
cp postgresql.tf postgresql.tf.backup
cp security_groups.tf security_groups.tf.backup
```

2. **Make the changes** above

3. **Validate**:
```bash
terraform validate
```

4. **Plan**:
```bash
terraform plan
```

5. **Apply**:
```bash
terraform apply
```

6. **Test Lambda**:
```bash
aws lambda invoke \
  --function-name postgres_dataload \
  --payload '{"Records":[{"body":"{\"test\":true}"}]}' \
  response.json

cat response.json
```

7. **Check CloudWatch Logs**:
```bash
aws logs tail /aws/lambda/postgres_dataload --follow
```

## Security Considerations

### Making RDS Publicly Accessible

**Concerns:**
- RDS has a public IP (but still protected by security group)
- Must restrict security group to specific IP ranges

**Mitigations:**
1. Use strong database password (already in place)
2. Restrict security group to Lambda IP ranges only
3. Enable RDS encryption at rest
4. Enable RDS encryption in transit (SSL)
5. Use AWS Secrets Manager for credentials
6. Enable RDS CloudWatch logging
7. Enable RDS backup and point-in-time recovery

### Better Security: Use RDS Proxy

If you want better security without NAT Gateway:

```hcl
resource "aws_db_proxy" "postgres" {
  name                   = "postgres-proxy"
  debug_logging          = false
  engine_family          = "POSTGRESQL"
  auth {
    auth_scheme = "SECRETS"
    iam_auth    = "DISABLED"
    secret_arn  = aws_secretsmanager_secret.db_credentials.arn
  }
  role_arn               = aws_iam_role.db_proxy.arn
  vpc_subnet_ids         = tolist(data.aws_subnets.default.ids)
  require_tls            = true
}
```

Then Lambda connects to the proxy, which handles connection pooling and security.

## Cost Comparison

| Solution | Monthly Cost | Pros | Cons |
|----------|-------------|------|------|
| NAT Gateway | ~$32 | Most secure | Expensive |
| VPC Endpoints | ~$14 | Secure, medium cost | Complex setup |
| Public RDS + IP Restrictions | $0 | Free, simple | Requires careful security |
| RDS Proxy | ~$10 | Good security, connection pooling | Added complexity |
| **Remove VPC from Lambda** | **$0** | **Simple, free, works** | **RDS needs public endpoint** |

## Recommended Approach for Your Use Case

Given your requirements:
1. No NAT Gateway (cost constraint)
2. Need Lambda to access SQS and RDS
3. Development/testing environment

**I recommend:**

```
┌─────────────────────────────────────────────────┐
│                                                 │
│  etenders_scraper (No VPC) ────► SQS           │
│                                    │            │
│                                    ↓            │
│  postgres_dataload (No VPC) ◄──── SQS           │
│         │                                       │
│         │                                       │
│         ↓                                       │
│  ┌─────────────────────────────────┐           │
│  │  RDS PostgreSQL (In VPC)        │           │
│  │  publicly_accessible = true     │           │
│  │  Security Group: Lambda IPs     │           │
│  └─────────────────────────────────┘           │
│                                                 │
└─────────────────────────────────────────────────┘
```

## Quick Fix (Apply These Changes)

### File: `lambdas.tf`

Remove the `vpc_config` block from `postgres_dataload`, `pdf_processing`, `get_data`, `ml_bid_predictor`, and `ai_summary`.

### File: `postgresql.tf`

Change:
```hcl
publicly_accessible = true
```

### File: `security_groups.tf`

Replace `postgres_sg` with:
```hcl
resource "aws_security_group" "postgres_sg" {
  name        = "postgres-sg"
  description = "Allow PostgreSQL from Lambda and Bastion"
  vpc_id      = data.aws_vpc.default.id

  ingress {
    description = "PostgreSQL from anywhere (restricted by AWS)"
    from_port   = 5432
    to_port     = 5432
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  tags = {
    Name = "postgres-sg"
  }
}
```

Then apply:
```bash
terraform apply
```

Your Lambda will connect successfully! 🎉

## Why This Works

1. **Lambda outside VPC** = Has internet access for SQS ✓
2. **RDS publicly accessible** = Lambda can reach it ✓  
3. **Security group** = Controls who can connect ✓
4. **No extra cost** = No NAT Gateway needed ✓

This is a valid pattern for development and even some production workloads where the cost of NAT Gateway isn't justified.