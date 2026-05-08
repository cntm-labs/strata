variable "region" {
  description = "AWS region"
  type        = string
  default     = "us-east-1"
}

variable "environment" {
  description = "Environment label (used in name prefixes and STRATA_ENV)"
  type        = string
  default     = "prod"
}

# === Networking ===

variable "vpc_cidr" {
  description = "VPC CIDR block"
  type        = string
  default     = "10.0.0.0/16"
}

variable "single_nat_gateway" {
  description = "Use one NAT Gateway shared across AZs (saves ~$45/mo, no HA)"
  type        = bool
  default     = false
}

# === Database ===

variable "multi_az" {
  description = "RDS Multi-AZ deployment"
  type        = bool
  default     = true
}

variable "rds_instance_class" {
  description = "RDS instance class"
  type        = string
  default     = "db.t4g.small"
}

# === Edge ===

variable "acm_certificate_arn" {
  description = "ACM certificate ARN for the HTTPS listener (must be in same region as ALB)"
  type        = string
}

# === Compute ===

variable "image_tag" {
  description = "Container image tag (override per deploy)"
  type        = string
  default     = "latest"
}

variable "desired_count" {
  description = "Number of running ECS tasks"
  type        = number
  default     = 2
}

# === Secrets — pass via TF_VAR_* env vars, never via .tfvars files ===

variable "nucleus_secret_key" {
  description = "Nucleus JWT signing key"
  type        = string
  sensitive   = true
  default     = ""
}

variable "resend_api_key" {
  description = "Resend API key"
  type        = string
  sensitive   = true
  default     = ""
}

variable "sentry_dsn" {
  description = "Sentry DSN"
  type        = string
  sensitive   = true
  default     = ""
}

# === Monitoring ===

variable "alarm_email" {
  description = "Email subscribed to CloudWatch alarms"
  type        = string
}
