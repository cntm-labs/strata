variable "name" {
  description = "Name prefix"
  type        = string
}

variable "vpc_id" {
  description = "VPC ID"
  type        = string
}

variable "subnet_ids" {
  description = "Public subnet IDs (one per AZ)"
  type        = list(string)
}

variable "security_group_ids" {
  description = "ALB security group IDs"
  type        = list(string)
}

variable "acm_certificate_arn" {
  description = "ACM certificate ARN for the HTTPS listener — must be in the same region as the ALB"
  type        = string
}

variable "health_check_path" {
  description = "Health check path on the strata service"
  type        = string
  default     = "/api/v1/health"
}
