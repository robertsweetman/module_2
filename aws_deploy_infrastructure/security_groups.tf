# Create a security group for the RDS instance
resource "aws_security_group" "postgres_sg" {
  name        = "postgres-sg"
  description = "Allow PostgreSQL inbound traffic"

  # Allow direct internet access
  ingress {
    description = "PostgreSQL from Internet"
    from_port   = 5432
    to_port     = 5432
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }

  tags = {
    Name = "postgres-sg"
  }
}

# Use default VPC
data "aws_vpc" "default" {
  default = true
}

# Create a subnet group for the RDS instance
resource "aws_db_subnet_group" "postgres" {
  name       = "postgres-subnet-group"
  subnet_ids = tolist(data.aws_subnets.default.ids)

  tags = {
    Name = "Postgres subnet group"
  }
}