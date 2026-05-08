variable "name" {
  description = "Name prefix"
  type        = string
}

variable "subnet_ids" {
  description = "Private subnet IDs — must span at least 2 AZs for Multi-AZ"
  type        = list(string)
}

variable "security_group_ids" {
  description = "RDS security group IDs (typically the rds_sg from the vpc module)"
  type        = list(string)
}

variable "master_password_secret_arn" {
  description = "Secrets Manager ARN holding the master password"
  type        = string
}

variable "db_name" {
  description = "Initial database name"
  type        = string
  default     = "strata"
}

variable "instance_class" {
  description = "RDS instance class"
  type        = string
  default     = "db.t4g.small"
}

variable "multi_az" {
  description = "Enable Multi-AZ deployment"
  type        = bool
  default     = true
}
