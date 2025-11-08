# Security group for Lambda functions (no cross-references in inline rules)
resource "aws_security_group" "lambda_sg" {
  name        = "lambda-sg"
  description = "Security group for Lambda functions"
  vpc_id      = data.aws_vpc.default.id

  # Allow HTTPS outbound for external API calls (e.g., Anthropic API)
  egress {
    description = "HTTPS outbound"
    from_port   = 443
    to_port     = 443
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  # Allow HTTP outbound for any HTTP API calls
  egress {
    description = "HTTP outbound"
    from_port   = 80
    to_port     = 80
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  tags = {
    Name = "lambda-sg"
  }
}

# Security group for bastion host (no cross-references in inline rules)
resource "aws_security_group" "bastion_sg" {
  name        = "bastion-sg"
  description = "Security group for bastion host with SSM access"
  vpc_id      = data.aws_vpc.default.id

  # Allow HTTPS outbound for SSM
  egress {
    description = "HTTPS outbound for SSM"
    from_port   = 443
    to_port     = 443
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  tags = {
    Name = "bastion-sg"
  }
}

# Create a security group for the RDS instance (no cross-references in inline rules)
resource "aws_security_group" "postgres_sg" {
  name        = "postgres-sg"
  description = "Allow PostgreSQL inbound traffic from Lambda and Bastion only"
  vpc_id      = data.aws_vpc.default.id

  tags = {
    Name = "postgres-sg"
  }
}

# Separate security group rules to avoid circular dependencies

# Allow Lambda to connect to RDS
resource "aws_security_group_rule" "lambda_to_postgres" {
  type                     = "egress"
  from_port                = 5432
  to_port                  = 5432
  protocol                 = "tcp"
  source_security_group_id = aws_security_group.postgres_sg.id
  security_group_id        = aws_security_group.lambda_sg.id
  description              = "PostgreSQL to RDS"
}

# Allow Bastion to connect to RDS
resource "aws_security_group_rule" "bastion_to_postgres" {
  type                     = "egress"
  from_port                = 5432
  to_port                  = 5432
  protocol                 = "tcp"
  source_security_group_id = aws_security_group.postgres_sg.id
  security_group_id        = aws_security_group.bastion_sg.id
  description              = "PostgreSQL to RDS"
}

# Allow RDS to receive connections from Lambda
resource "aws_security_group_rule" "postgres_from_lambda" {
  type                     = "ingress"
  from_port                = 5432
  to_port                  = 5432
  protocol                 = "tcp"
  source_security_group_id = aws_security_group.lambda_sg.id
  security_group_id        = aws_security_group.postgres_sg.id
  description              = "PostgreSQL from Lambda"
}

# Allow RDS to receive connections from Bastion
resource "aws_security_group_rule" "postgres_from_bastion" {
  type                     = "ingress"
  from_port                = 5432
  to_port                  = 5432
  protocol                 = "tcp"
  source_security_group_id = aws_security_group.bastion_sg.id
  security_group_id        = aws_security_group.postgres_sg.id
  description              = "PostgreSQL from Bastion"
}

# Use default VPC
data "aws_vpc" "default" {
  default = true
}

# Get Internet Gateway for the default VPC
data "aws_internet_gateway" "default" {
  filter {
    name   = "attachment.vpc-id"
    values = [data.aws_vpc.default.id]
  }
}

# Create a subnet group for the RDS instance
resource "aws_db_subnet_group" "postgres" {
  name       = "postgres-subnet-group"
  subnet_ids = tolist(data.aws_subnets.default.ids)

  tags = {
    Name = "Postgres subnet group"
  }
}

# IAM role for EC2 to use SSM
resource "aws_iam_role" "bastion_role" {
  name = "bastion-ssm-role"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Action = "sts:AssumeRole"
        Effect = "Allow"
        Principal = {
          Service = "ec2.amazonaws.com"
        }
      }
    ]
  })

  tags = {
    Name = "bastion-ssm-role"
  }
}

# Attach SSM managed policy to the role
resource "aws_iam_role_policy_attachment" "bastion_ssm_policy" {
  role       = aws_iam_role.bastion_role.name
  policy_arn = "arn:aws:iam::aws:policy/AmazonSSMManagedInstanceCore"
}

# Instance profile for the bastion host
resource "aws_iam_instance_profile" "bastion_profile" {
  name = "bastion-profile"
  role = aws_iam_role.bastion_role.name
}

# Get the latest Amazon Linux 2 AMI
data "aws_ami" "amazon_linux" {
  most_recent = true
  owners      = ["amazon"]

  filter {
    name   = "name"
    values = ["amzn2-ami-hvm-*-x86_64-gp2"]
  }

  filter {
    name   = "virtualization-type"
    values = ["hvm"]
  }
}

# Bastion host EC2 instance
resource "aws_instance" "bastion" {
  ami                    = data.aws_ami.amazon_linux.id
  instance_type          = "t3.micro" # Small, cost-effective instance
  vpc_security_group_ids = [aws_security_group.bastion_sg.id]
  iam_instance_profile   = aws_iam_instance_profile.bastion_profile.name

  # Use the first available subnet
  subnet_id = tolist(data.aws_subnets.default.ids)[0]

  # Install PostgreSQL client
  user_data = <<-EOF
    #!/bin/bash
    yum update -y
    yum install -y postgresql

    # Install AWS CLI v2
    curl "https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip" -o "awscliv2.zip"
    yum install -y unzip
    unzip awscliv2.zip
    ./aws/install

    # Create a connection script for easy database access
    cat > /home/ec2-user/connect-db.sh << 'SCRIPT'
#!/bin/bash
echo "Connecting to PostgreSQL database..."
echo "Use the following connection details:"
echo "Host: ${aws_db_instance.postgres.endpoint}"
echo "Port: 5432"
echo "Database: ${var.db_name}"
echo "Username: ${var.db_admin_name}"
echo ""
echo "Example connection command:"
echo "psql -h ${aws_db_instance.postgres.endpoint} -p 5432 -U ${var.db_admin_name} -d ${var.db_name}"
SCRIPT

    chmod +x /home/ec2-user/connect-db.sh
    chown ec2-user:ec2-user /home/ec2-user/connect-db.sh
  EOF

  tags = {
    Name    = "PostgreSQL-Bastion-Host"
    Purpose = "Database access via SSM Session Manager"
  }
}
