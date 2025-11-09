# RDS PostgreSQL Connectivity Fix

## Problem

Lambda function `postgres_dataload` was timing out when trying to connect to the RDS PostgreSQL database with the error:

```
Failed to connect to database: error communicating with database: Operation timed out (os error 110)
```

## Root Cause

When a Lambda function is placed in a VPC (even the default VPC), it **loses direct internet access**. The `postgres_dataload` Lambda function needed to:

1. ✅ Connect to RDS PostgreSQL (VPC access) 
2. ❌ Connect to SQS (requires internet access or VPC endpoints)

Without proper networking configuration, the Lambda couldn't reach AWS services like SQS through the internet.

## Solution: VPC Endpoints

We implemented **VPC Endpoints** to allow Lambda functions to access AWS services without requiring internet access. This is the recommended AWS best practice for Lambda functions in VPCs.

### What We Added

#### 1. SQS VPC Endpoint (`vpc_endpoints.tf`)

```hcl
resource "aws_vpc_endpoint" "sqs" {
  vpc_id              = data.aws_vpc.default.id
  service_name        = "com.amazonaws.${data.aws_region.current.name}.sqs"
  vpc_endpoint_type   = "Interface"
  subnet_ids          = tolist(data.aws_subnets.default.ids)
  security_group_ids  = [aws_security_group.vpc_endpoint_sg.id]
  private_dns_enabled = true
}
```

**Benefits:**
- Allows Lambda to communicate with SQS privately within the VPC
- No NAT Gateway required (saves ~$32/month)
- Lower latency
- More secure (traffic stays within AWS network)

#### 2. Secrets Manager VPC Endpoint

```hcl
resource "aws_vpc_endpoint" "secrets_manager" {
  vpc_id              = data.aws_vpc.default.id
  service_name        = "com.amazonaws.${data.aws_region.current.name}.secretsmanager"
  vpc_endpoint_type   = "Interface"
  subnet_ids          = tolist(data.aws_subnets.default.ids)
  security_group_ids  = [aws_security_group.vpc_endpoint_sg.id]
  private_dns_enabled = true
}
```

**Purpose:** For future secure credential management if needed.

#### 3. S3 Gateway VPC Endpoint

```hcl
resource "aws_vpc_endpoint" "s3" {
  vpc_id            = data.aws_vpc.default.id
  service_name      = "com.amazonaws.${data.aws_region.current.name}.s3"
  vpc_endpoint_type = "Gateway"
  route_table_ids   = [data.aws_route_table.main.id]
}
```

**Benefits:**
- Gateway endpoints are **FREE** (no hourly or data processing charges)
- Allows Lambda to access S3 without internet
- Required for PDF processing pipeline

#### 4. VPC Endpoint Security Group

```hcl
resource "aws_security_group" "vpc_endpoint_sg" {
  name        = "vpc-endpoint-sg"
  description = "Security group for VPC endpoints"
  vpc_id      = data.aws_vpc.default.id

  ingress {
    description     = "HTTPS from Lambda"
    from_port       = 443
    to_port         = 443
    protocol        = "tcp"
    security_groups = [aws_security_group.lambda_sg.id]
  }

  egress {
    description = "Allow all outbound"
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }
}
```

**Purpose:** Controls access to VPC endpoints, allowing only Lambda functions to communicate.

## Network Flow After Fix

```
┌─────────────────────────────────────────────────────────┐
│                       Default VPC                        │
│                                                          │
│  ┌──────────────────┐                                   │
│  │ etenders_scraper │ (No VPC - has internet access)    │
│  └────────┬─────────┘                                   │
│           │                                              │
│           │ Sends messages                               │
│           ↓                                              │
│  ┌─────────────────┐         ┌──────────────────────┐  │
│  │   SQS Queue     │◄────────┤  SQS VPC Endpoint    │  │
│  │ tender-processing│         └──────────▲───────────┘  │
│  └─────────────────┘                    │               │
│           │                             │               │
│           │ Triggers                    │ Accesses      │
│           ↓                             │               │
│  ┌──────────────────┐                  │               │
│  │postgres_dataload │──────────────────┘               │
│  │   (In VPC)       │                                   │
│  └────────┬─────────┘                                   │
│           │                                              │
│           │ Connects via port 5432                       │
│           ↓                                              │
│  ┌──────────────────┐                                   │
│  │  RDS PostgreSQL  │                                   │
│  │  (Private)       │                                   │
│  └──────────────────┘                                   │
└─────────────────────────────────────────────────────────┘
```

## Security Improvements

1. **No Internet Gateway Required**: Lambda functions don't need internet access, reducing attack surface
2. **Private Traffic**: All communication stays within AWS network
3. **Security Group Controls**: Fine-grained access control between services
4. **Private RDS**: Database remains inaccessible from internet

## Cost Analysis

### This Solution (VPC Endpoints)
- **SQS Interface Endpoint**: ~$7.20/month + $0.01/GB data processed
- **Secrets Manager Interface Endpoint**: ~$7.20/month + $0.01/GB data processed
- **S3 Gateway Endpoint**: **FREE** ✅
- **Total**: ~$14.40/month + minimal data transfer costs

### Alternative Solution (NAT Gateway)
- **NAT Gateway**: ~$32.40/month + $0.045/GB data processed
- **Elastic IP**: ~$3.60/month (if not attached)
- **Total**: ~$36/month + higher data transfer costs

**Savings: ~$21.60/month (60% cheaper)**

## Deployment Steps

1. **Review the changes**:
   ```bash
   cd module_2/aws_deploy_infrastructure
   terraform plan
   ```

2. **Apply the infrastructure**:
   ```bash
   terraform apply
   ```

3. **Verify VPC Endpoints are created**:
   ```bash
   aws ec2 describe-vpc-endpoints --filters "Name=vpc-id,Values=<your-vpc-id>"
   ```

4. **Test the Lambda function**:
   - Trigger the `etenders_scraper` Lambda manually or wait for scheduled run
   - Monitor CloudWatch Logs for `postgres_dataload` Lambda
   - Should see successful database connections

## Monitoring

### CloudWatch Logs to Watch

1. **postgres_dataload Lambda**:
   - Log Group: `/aws/lambda/postgres_dataload`
   - Look for: "=== POSTGRES DATALOAD STARTED ==="
   - Verify: No timeout errors

2. **VPC Endpoint Metrics**:
   - Check data transfer through endpoints
   - Monitor for any connection errors

### Success Indicators

✅ Lambda connects to RDS without timeout  
✅ Lambda can send/receive SQS messages  
✅ Lambda can access S3 buckets for PDFs  
✅ No "Operation timed out" errors  
✅ Tenders are successfully processed and stored  

## Troubleshooting

### If Lambda Still Times Out

1. **Check Security Groups**:
   ```bash
   # Verify Lambda can reach VPC endpoints
   aws ec2 describe-security-groups --group-ids <lambda-sg-id>
   ```

2. **Verify VPC Endpoint DNS**:
   - Ensure `private_dns_enabled = true` for Interface endpoints
   - Lambda should resolve AWS service endpoints to private IPs

3. **Check RDS Security Group**:
   ```bash
   # Verify Lambda SG is allowed on port 5432
   aws ec2 describe-security-groups --group-ids <postgres-sg-id>
   ```

4. **Test Database Connection from Bastion**:
   ```bash
   # Connect via SSM
   aws ssm start-session --target <bastion-instance-id>
   
   # Test PostgreSQL connection
   psql -h <rds-endpoint> -U <username> -d <database>
   ```

### Common Issues

| Issue | Cause | Solution |
|-------|-------|----------|
| Still timing out on SQS | VPC endpoint not created | Run `terraform apply` |
| Can't reach RDS | Security group rules missing | Check `postgres_from_lambda` rule exists |
| DNS resolution fails | `private_dns_enabled = false` | Set to `true` in VPC endpoint config |
| S3 access fails | Route table not configured | Verify S3 endpoint in route table |

## Alternative Solutions (Not Implemented)

### Option 1: NAT Gateway (More Expensive)
- **Pros**: Simpler, Lambda gets full internet access
- **Cons**: ~$32/month extra cost, less secure
- **When to use**: If Lambda needs to call many external APIs

### Option 2: Remove Lambda from VPC
- **Pros**: Lambda gets internet access automatically
- **Cons**: Can't connect to RDS (RDS must be public - security risk!)
- **When to use**: Never for production databases

### Option 3: VPC Peering
- **Pros**: Can connect multiple VPCs
- **Cons**: Overly complex for this use case
- **When to use**: Multi-VPC architectures

## Best Practices Implemented

✅ **Separation of Concerns**: `etenders_scraper` (no VPC) handles web scraping, `postgres_dataload` (in VPC) handles database  
✅ **Least Privilege**: Security groups only allow necessary ports  
✅ **Cost Optimization**: Using free S3 gateway endpoint  
✅ **Private Database**: RDS not publicly accessible  
✅ **Event-Driven**: SQS decouples scraping from processing  
✅ **Scalability**: VPC endpoints automatically scale  

## References

- [AWS Lambda VPC Networking](https://docs.aws.amazon.com/lambda/latest/dg/configuration-vpc.html)
- [VPC Endpoints](https://docs.aws.amazon.com/vpc/latest/privatelink/vpc-endpoints.html)
- [Lambda Best Practices](https://docs.aws.amazon.com/lambda/latest/dg/best-practices.html)

## Next Steps

1. ✅ Apply Terraform changes
2. ⬜ Monitor Lambda execution for 24-48 hours
3. ⬜ Verify tender scraping and processing pipeline
4. ⬜ Set up CloudWatch alarms for timeouts
5. ⬜ Document any edge cases discovered
6. ⬜ Consider adding more VPC endpoints as needed (e.g., CloudWatch Logs)

---

**Last Updated**: 2024
**Status**: Ready for deployment
**Tested**: Terraform validation passed ✅