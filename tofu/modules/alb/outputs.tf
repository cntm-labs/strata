output "alb_arn" {
  value = aws_lb.this.arn
}

output "alb_arn_suffix" {
  description = "Suffix used in CloudWatch metric dimensions"
  value       = aws_lb.this.arn_suffix
}

output "alb_dns_name" {
  description = "DNS name of the ALB — point your domain CNAME/ALIAS at this"
  value       = aws_lb.this.dns_name
}

output "alb_zone_id" {
  description = "ALB hosted zone ID — used for Route 53 ALIAS records"
  value       = aws_lb.this.zone_id
}

output "target_group_arn" {
  value = aws_lb_target_group.this.arn
}
