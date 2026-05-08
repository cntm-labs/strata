output "endpoint" {
  description = "RDS endpoint in host:port form"
  value       = aws_db_instance.this.endpoint
}

output "address" {
  description = "RDS hostname (no port)"
  value       = aws_db_instance.this.address
}

output "port" {
  value = aws_db_instance.this.port
}

output "db_name" {
  value = aws_db_instance.this.db_name
}

output "instance_arn" {
  value = aws_db_instance.this.arn
}
