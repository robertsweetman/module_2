# Create the PostgreSQL RDS instance
resource "aws_db_instance" "postgres" {
  identifier          = "postgres-db"
  engine              = "postgres"
  engine_version      = "17.5"        # Choose appropriate version
  instance_class      = "db.t3.micro" # Choose appropriate size
  allocated_storage   = 20
  storage_type        = "gp2"
  db_name             = var.db_name
  username            = var.db_admin_name
  password            = var.db_admin_pwd
  skip_final_snapshot = true  # Set to false for production
  publicly_accessible = false # Set to false for production

  vpc_security_group_ids = [aws_security_group.postgres_sg.id]
  db_subnet_group_name   = aws_db_subnet_group.postgres.name

  tags = {
    Name        = "PostgreSQL Database"
    Environment = "Development"
    Application = "eTenders"
  }
}
