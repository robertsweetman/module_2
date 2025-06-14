# Create an Elastic IP for the NAT Gateway
resource "aws_eip" "nat_eip" {
  domain = "vpc"
}

# Get all subnets (which are public in the default VPC)
data "aws_subnets" "all" {
  filter {
    name   = "vpc-id"
    values = [data.aws_vpc.default.id]
  }
}

# Create a NAT Gateway in a public subnet
resource "aws_nat_gateway" "main" {
  allocation_id = aws_eip.nat_eip.id
  subnet_id     = tolist(data.aws_subnets.all.ids)[0]  # Use first subnet for NAT Gateway
  
  tags = {
    Name = "main-nat-gateway"
  }
}

# Create a route table for Lambda functions
resource "aws_route_table" "lambda" {
  vpc_id = data.aws_vpc.default.id
  
  route {
    cidr_block     = "0.0.0.0/0"
    nat_gateway_id = aws_nat_gateway.main.id
  }
  
  tags = {
    Name = "lambda-route-table"
  }
}

# Associate the route table with all subnets
resource "aws_route_table_association" "lambda" {
  count          = length(data.aws_subnets.all.ids)
  subnet_id      = tolist(data.aws_subnets.all.ids)[count.index]
  route_table_id = aws_route_table.lambda.id
}
