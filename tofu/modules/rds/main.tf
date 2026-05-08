# Multi-AZ Postgres 16 instance with encrypted storage and 7-day backups.
# Master password comes from Secrets Manager (managed by the secrets
# module). The app reads its own DATABASE_URL secret at task start.

resource "aws_db_subnet_group" "this" {
  name       = "${var.name}-rds"
  subnet_ids = var.subnet_ids
  tags       = { Name = "${var.name}-rds" }
}

resource "aws_db_parameter_group" "this" {
  name   = "${var.name}-postgres16"
  family = "postgres16"

  parameter {
    name         = "max_connections"
    value        = "200"
    apply_method = "pending-reboot"
  }

  parameter {
    name  = "log_statement"
    value = "ddl"
  }
}

# Read the master password from Secrets Manager. Terraform reads this
# once at apply time; subsequent rotation through Secrets Manager is
# transparent to the DB.
data "aws_secretsmanager_secret_version" "master_password" {
  secret_id = var.master_password_secret_arn
}

resource "aws_db_instance" "this" {
  identifier     = "${var.name}-postgres"
  engine         = "postgres"
  engine_version = "16.4"
  instance_class = var.instance_class

  allocated_storage     = 20
  max_allocated_storage = 100
  storage_type          = "gp3"
  storage_encrypted     = true

  db_name  = var.db_name
  username = "strata"
  password = data.aws_secretsmanager_secret_version.master_password.secret_string

  db_subnet_group_name   = aws_db_subnet_group.this.name
  vpc_security_group_ids = var.security_group_ids
  parameter_group_name   = aws_db_parameter_group.this.name

  multi_az            = var.multi_az
  publicly_accessible = false

  backup_retention_period = 7
  backup_window           = "03:00-04:00"
  maintenance_window      = "sun:04:00-sun:05:00"

  deletion_protection       = true
  skip_final_snapshot       = false
  final_snapshot_identifier = "${var.name}-final"

  apply_immediately     = false
  copy_tags_to_snapshot = true

  tags = { Name = "${var.name}-postgres" }
}
