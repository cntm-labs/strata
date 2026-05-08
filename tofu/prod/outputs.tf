output "alb_dns_name" {
  description = "Point your domain at this CNAME / ALIAS"
  value       = module.alb.alb_dns_name
}

output "alb_zone_id" {
  description = "ALB hosted zone ID — use for Route 53 ALIAS records"
  value       = module.alb.alb_zone_id
}

output "rds_endpoint" {
  value = module.rds.endpoint
}

output "ecs_cluster_arn" {
  value = module.ecs.cluster_arn
}

output "dashboard_url" {
  value = module.monitoring.dashboard_url
}
