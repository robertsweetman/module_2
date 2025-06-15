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

# Create a default route table for the VPC
resource "aws_route_table" "default" {
  vpc_id = data.aws_vpc.default.id

  tags = {
    Name = "default-route-table"
  }
}

# Associate the default route table with all subnets
resource "aws_route_table_association" "default" {
  count          = length(data.aws_subnets.default.ids)
  subnet_id      = tolist(data.aws_subnets.default.ids)[count.index]
  route_table_id = aws_route_table.default.id
}