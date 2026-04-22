variable "vpc_id" {}
variable "subnets" {
  type = list(string)
}
variable "db_name" {}

output "connection_url" {
  value = "postgresql://strata_admin:password@db-endpoint:5432/strata"
}
