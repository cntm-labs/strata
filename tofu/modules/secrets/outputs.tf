output "postgres_password_secret_arn" {
  value = aws_secretsmanager_secret.postgres_password.arn
}

output "strata_app_password_secret_arn" {
  value = aws_secretsmanager_secret.strata_app_password.arn
}

# Plain-text password values are NOT exported. Consumers (RDS, the prod
# composition's DATABASE_URL secrets) read values via
# `data.aws_secretsmanager_secret_version` against the ARNs above.

output "nucleus_secret_arn" {
  value = aws_secretsmanager_secret.nucleus.arn
}

output "resend_secret_arn" {
  value = aws_secretsmanager_secret.resend.arn
}

output "sentry_dsn_secret_arn" {
  value = aws_secretsmanager_secret.sentry_dsn.arn
}
