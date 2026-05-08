variable "name" {
  description = "Name prefix"
  type        = string
}

variable "region" {
  description = "AWS region (used in awslogs config)"
  type        = string
}

variable "vpc_id" {
  description = "VPC ID"
  type        = string
}

variable "subnet_ids" {
  description = "Private subnet IDs"
  type        = list(string)
}

variable "security_group_ids" {
  description = "ECS task security group IDs"
  type        = list(string)
}

variable "target_group_arn" {
  description = "ALB target group ARN — service registers here"
  type        = string
}

variable "image" {
  description = "Container image (without tag)"
  type        = string
  default     = "ghcr.io/cntm-labs/strata"
}

variable "image_tag" {
  description = "Container image tag"
  type        = string
  default     = "latest"
}

variable "cpu" {
  description = "Fargate CPU units (256, 512, 1024, 2048, 4096)"
  type        = number
  default     = 512
}

variable "memory" {
  description = "Fargate memory in MiB"
  type        = number
  default     = 1024
}

variable "desired_count" {
  description = "Number of running tasks"
  type        = number
  default     = 2
}

variable "strata_env" {
  description = "Value for the STRATA_ENV environment variable (used by Sentry tags)"
  type        = string
  default     = "production"
}

variable "secret_arns" {
  description = "Flat list of all Secrets Manager ARNs the task needs read access to"
  type        = list(string)
}

variable "secret_arns_by_name" {
  description = "Per-secret ARNs keyed by short name — used in the task definition's secrets[] block"
  type = object({
    database_url_admin  = string
    database_url        = string
    strata_app_password = string
    nucleus             = string
    resend              = string
    sentry_dsn          = string
  })
}
