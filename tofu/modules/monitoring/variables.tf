variable "name" {
  description = "Name prefix"
  type        = string
}

variable "region" {
  description = "AWS region (for dashboard widgets)"
  type        = string
}

variable "alarm_email" {
  description = "Email subscribed to the SNS alarm topic"
  type        = string
}

variable "alb_arn_suffix" {
  description = "ALB arn_suffix for CloudWatch metric dimensions"
  type        = string
}

variable "ecs_cluster_name" {
  description = "ECS cluster name (for ContainerInsights metrics)"
  type        = string
}

variable "ecs_service_name" {
  description = "ECS service name"
  type        = string
}

variable "log_group_name" {
  description = "CloudWatch log group name (for dashboard log widget)"
  type        = string
}

variable "desired_count" {
  description = "Expected ECS task count — task-count alarm threshold"
  type        = number
}
