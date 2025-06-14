# Create an Elastic IP for the NAT Gateway
resource "aws_eip" "nat_eip" {
  domain = "vpc"
}

# Identify public subnets for NAT Gateway
data "aws_subnets" "public" {
  filter {
    name   = "vpc-id"
    values = [data.aws_vpc.default.id]
  }
  filter {
    name   = "map-public-ip-on-launch"
    values = ["true"]
  }
}

# Create a NAT Gateway in a public subnet
resource "aws_nat_gateway" "main" {
  allocation_id = aws_eip.nat_eip.id
  subnet_id     = tolist(data.aws_subnets.public.ids)[0]
  
  tags = {
    Name = "main-nat-gateway"
  }
}

# Create a route table for private subnets
resource "aws_route_table" "private" {
  vpc_id = data.aws_vpc.default.id
  
  route {
    cidr_block     = "0.0.0.0/0"
    nat_gateway_id = aws_nat_gateway.main.id
  }
  
  tags = {
    Name = "private-route-table"
  }
}

# Get private subnets (those that are not public)
data "aws_subnets" "private" {
  filter {
    name   = "vpc-id"
    values = [data.aws_vpc.default.id]
  }
  filter {
    name   = "map-public-ip-on-launch"
    values = ["false"]
  }
}

# Associate the route table only with private subnets
resource "aws_route_table_association" "private" {
  count          = length(data.aws_subnets.all.ids)
  subnet_id      = tolist(data.aws_subnets.all.ids)[count.index]
  route_table_id = aws_route_table.private.id
}
