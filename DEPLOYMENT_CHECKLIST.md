# Deployment Checklist: RDS Connectivity Fix

## Pre-Deployment

- [ ] Review all Terraform changes
  ```bash
  cd module_2/aws_deploy_infrastructure
  terraform fmt
  terraform validate
  ```

- [ ] Check current infrastructure state
  ```bash
  terraform plan
  ```

- [ ] Backup current Terraform state
  ```bash
  cp terraform.tfstate terraform.tfstate.backup.$(date +%Y%m%d_%H%M%S)
  ```

- [ ] Document current Lambda timeout counts
  ```bash
  aws cloudwatch get-metric-statistics \
    --namespace AWS/Lambda \
    --metric-name Errors \
    --dimensions Name=FunctionName,Value=postgres_dataload \
    --start-time $(date -u -d '1 hour ago' +%Y-%m-%dT%H:%M:%S) \
    --end-time $(date -u +%Y-%m-%dT%H:%M:%S) \
    --period 3600 \
    --statistics Sum
  ```

## Deployment Steps

### 1. Apply Infrastructure Changes

- [ ] Deploy VPC endpoints
  ```bash
  terraform apply -target=aws_security_group.vpc_endpoint_sg
  terraform apply -target=aws_vpc_endpoint.sqs
  terraform apply -target=aws_vpc_endpoint.secrets_manager
  terraform apply -target=aws_vpc_endpoint.s3
  ```

- [ ] Verify VPC endpoints are created
  ```bash
  aws ec2 describe-vpc-endpoints --filters "Name=tag:Name,Values=sqs-vpc-endpoint"
  aws ec2 describe-vpc-endpoints --filters "Name=tag:Name,Values=s3-vpc-endpoint"
  ```

- [ ] Check VPC endpoint status (should be "available")
  ```bash
  aws ec2 describe-vpc-endpoints --query 'VpcEndpoints[*].[VpcEndpointId,State,ServiceName]' --output table
  ```

### 2. Update Lambda Configuration (if needed)

- [ ] Lambda functions should automatically use VPC endpoints via DNS
- [ ] No code changes required (private DNS enabled)
- [ ] Verify Lambda security groups allow HTTPS (port 443) outbound

### 3. Test Database Connectivity

- [ ] Connect to bastion host via SSM
  ```bash
  aws ssm start-session --target <bastion-instance-id>
  ```

- [ ] Test PostgreSQL connection from bastion
  ```bash
  psql -h <rds-endpoint> -p 5432 -U <username> -d <database> -c "SELECT 1;"
  ```

### 4. Test Lambda Functions

- [ ] Manually invoke `etenders_scraper` Lambda
  ```bash
  aws lambda invoke \
    --function-name etenders_scraper \
    --payload '{"max_pages":1,"test_mode":true,"start_page":1}' \
    response.json
  ```

- [ ] Check SQS queue for messages
  ```bash
  aws sqs get-queue-attributes \
    --queue-url <tender-processing-queue-url> \
    --attribute-names ApproximateNumberOfMessages
  ```

- [ ] Monitor `postgres_dataload` Lambda logs
  ```bash
  aws logs tail /aws/lambda/postgres_dataload --follow
  ```

- [ ] Verify successful database connection (no timeout errors)

### 5. Verify End-to-End Pipeline

- [ ] Check CloudWatch Logs for all Lambda functions
  - `/aws/lambda/etenders_scraper`
  - `/aws/lambda/postgres_dataload`
  - `/aws/lambda/pdf_processing`
  - `/aws/lambda/ml_bid_predictor`
  - `/aws/lambda/ai_summary`
  - `/aws/lambda/sns_notification`

- [ ] Query database for new tender records
  ```sql
  SELECT COUNT(*) as new_tenders 
  FROM tenders 
  WHERE created_at > NOW() - INTERVAL '1 hour';
  ```

- [ ] Check all SQS queues for stuck messages
  ```bash
  for queue in tender-processing pdf-processing ml-prediction ai-summary sns-notification; do
    echo "Checking $queue-queue..."
    aws sqs get-queue-attributes \
      --queue-url $(aws sqs get-queue-url --queue-name "$queue-queue" --query 'QueueUrl' --output text) \
      --attribute-names ApproximateNumberOfMessages,ApproximateNumberOfMessagesNotVisible
  done
  ```

## Post-Deployment Monitoring

### Immediate (0-2 hours)

- [ ] Monitor Lambda error rates in CloudWatch
- [ ] Check for any timeout errors in logs
- [ ] Verify tender processing pipeline is working
- [ ] Check VPC endpoint data transfer metrics

### Short-term (2-24 hours)

- [ ] Monitor scheduled `etenders_scraper` execution at 11:00 UTC
- [ ] Verify daily tender batch is processed successfully
- [ ] Check Dead Letter Queues for any failed messages
  ```bash
  aws sqs get-queue-attributes \
    --queue-url <tender-processing-dlq-url> \
    --attribute-names ApproximateNumberOfMessages
  ```
- [ ] Review CloudWatch metrics for Lambda invocations and durations

### Medium-term (24-48 hours)

- [ ] Analyze VPC endpoint costs vs previous costs
- [ ] Review database connection pool metrics
- [ ] Check for any unusual patterns in tender processing
- [ ] Verify all downstream pipelines (PDF, ML, AI summary, notifications)

## Rollback Plan

If issues persist after deployment:

### Option 1: Quick Rollback (Remove VPC from Lambda)

**⚠️ WARNING: This makes RDS publicly accessible - NOT RECOMMENDED for production**

```hcl
# In lambdas.tf, comment out vpc_config blocks
# vpc_config {
#   subnet_ids         = tolist(data.aws_subnets.default.ids)
#   security_group_ids = [aws_security_group.lambda_sg.id]
# }
```

Then:
```bash
terraform apply
```

### Option 2: Add NAT Gateway (More expensive but reliable)

```hcl
# Create NAT Gateway
resource "aws_eip" "nat" {
  domain = "vpc"
}

resource "aws_nat_gateway" "main" {
  allocation_id = aws_eip.nat.id
  subnet_id     = tolist(data.aws_subnets.default.ids)[0]
}

# Update route table
resource "aws_route" "nat" {
  route_table_id         = data.aws_route_table.main.id
  destination_cidr_block = "0.0.0.0/0"
  nat_gateway_id         = aws_nat_gateway.main.id
}
```

### Option 3: Restore from Backup

```bash
# Restore previous state
cp terraform.tfstate.backup.<timestamp> terraform.tfstate
terraform apply
```

## Success Criteria

✅ No timeout errors in `postgres_dataload` Lambda logs  
✅ Tenders are successfully scraped and stored in database  
✅ All SQS queues are processing messages  
✅ RDS connection pool is stable  
✅ VPC endpoints show "available" status  
✅ PDF processing pipeline continues to work  
✅ No increase in Lambda error rates  
✅ Cost remains within expected range (~$15/month for endpoints)  

## Validation Queries

Run these SQL queries to verify system health:

```sql
-- Check recent tender processing
SELECT 
  DATE(created_at) as date,
  COUNT(*) as tender_count,
  COUNT(DISTINCT reference) as unique_tenders
FROM tenders
WHERE created_at > NOW() - INTERVAL '7 days'
GROUP BY DATE(created_at)
ORDER BY date DESC;

-- Check PDF processing status
SELECT 
  pdf_generated,
  COUNT(*) as count
FROM tenders
WHERE created_at > NOW() - INTERVAL '24 hours'
GROUP BY pdf_generated;

-- Check ML predictions
SELECT 
  COUNT(*) as total_tenders,
  COUNT(ml_bid_probability) as predicted_count,
  AVG(ml_bid_probability) as avg_probability
FROM tenders
WHERE created_at > NOW() - INTERVAL '24 hours';

-- Check for duplicate tenders (should be 0)
SELECT reference, COUNT(*) as count
FROM tenders
GROUP BY reference
HAVING COUNT(*) > 1;
```

## CloudWatch Alarms to Set Up

```bash
# Lambda timeout alarm
aws cloudwatch put-metric-alarm \
  --alarm-name postgres-dataload-timeout \
  --alarm-description "Alert when postgres_dataload times out" \
  --metric-name Duration \
  --namespace AWS/Lambda \
  --statistic Average \
  --period 300 \
  --threshold 850000 \
  --comparison-operator GreaterThanThreshold \
  --evaluation-periods 2 \
  --dimensions Name=FunctionName,Value=postgres_dataload

# Lambda error alarm
aws cloudwatch put-metric-alarm \
  --alarm-name postgres-dataload-errors \
  --alarm-description "Alert when postgres_dataload has errors" \
  --metric-name Errors \
  --namespace AWS/Lambda \
  --statistic Sum \
  --period 300 \
  --threshold 5 \
  --comparison-operator GreaterThanThreshold \
  --evaluation-periods 1 \
  --dimensions Name=FunctionName,Value=postgres_dataload

# SQS DLQ alarm
aws cloudwatch put-metric-alarm \
  --alarm-name tender-processing-dlq-messages \
  --alarm-description "Alert when messages appear in DLQ" \
  --metric-name ApproximateNumberOfMessagesVisible \
  --namespace AWS/SQS \
  --statistic Average \
  --period 300 \
  --threshold 1 \
  --comparison-operator GreaterThanThreshold \
  --evaluation-periods 1 \
  --dimensions Name=QueueName,Value=tender-processing-dlq
```

## Cost Monitoring

Track costs for first week:

```bash
# Get cost breakdown
aws ce get-cost-and-usage \
  --time-period Start=$(date -d '7 days ago' +%Y-%m-%d),End=$(date +%Y-%m-%d) \
  --granularity DAILY \
  --metrics UnblendedCost \
  --group-by Type=SERVICE \
  --filter file://filter.json
```

Expected costs:
- **VPC Endpoints (Interface)**: ~$7.20/month each × 2 = $14.40/month
- **VPC Endpoint (Gateway - S3)**: $0/month (FREE)
- **Data Transfer**: ~$0.01/GB

## Notes

- VPC endpoints take 5-10 minutes to become available
- Lambda DNS resolution may be cached for up to 60 seconds
- First Lambda invocation after deployment may be slower (cold start)
- Monitor for 48 hours to ensure stability
- Document any issues in GitHub Issues or incident log

## Sign-off

- [ ] Infrastructure deployed successfully
- [ ] Tests passed
- [ ] Monitoring enabled
- [ ] Documentation updated
- [ ] Team notified

**Deployed by**: ___________________  
**Date**: ___________________  
**Time**: ___________________  
**Status**: ___________________