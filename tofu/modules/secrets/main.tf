# Secrets Manager entries the strata-server reads at startup. ECS task
# definition references these via the `secrets` block, which AWS injects
# as env vars at task start. No secret value appears in plan output or
# task definition JSON.
#
# This module manages the 5 "value-only" secrets that have no
# dependency on RDS or any other module:
#   - postgres_password / strata_app_password (random_password generated)
#   - nucleus_secret_key / resend_api_key / sentry_dsn (operator vars)
#
# DATABASE_URL_ADMIN and DATABASE_URL are composed inline in
# tofu/prod/main.tf — they need module.rds.endpoint *and* the
# random_password values, which would create a cross-module cycle if
# owned here (secrets → rds → secrets).
#
# Operator-provided secrets (NUCLEUS, RESEND, SENTRY) come in as
# sensitive Terraform variables — pass them via TF_VAR_* env, never via
# terraform.tfvars.

resource "random_password" "postgres" {
  length  = 48
  special = true
  # Exclude characters that need quoting in connection URLs or shell.
  override_special = "!#%^*-_=+:?"
}

resource "random_password" "strata_app" {
  length           = 48
  special          = true
  override_special = "!#%^*-_=+:?"
}

resource "aws_secretsmanager_secret" "postgres_password" {
  name        = "${var.name_prefix}/postgres-password"
  description = "RDS master password (used by strata-server admin pool + backup sidecar)"
}

resource "aws_secretsmanager_secret_version" "postgres_password" {
  secret_id     = aws_secretsmanager_secret.postgres_password.id
  secret_string = random_password.postgres.result
}

resource "aws_secretsmanager_secret" "strata_app_password" {
  name        = "${var.name_prefix}/strata-app-password"
  description = "Password for the strata_app non-super DB role (RLS-enforced)"
}

resource "aws_secretsmanager_secret_version" "strata_app_password" {
  secret_id     = aws_secretsmanager_secret.strata_app_password.id
  secret_string = random_password.strata_app.result
}

resource "aws_secretsmanager_secret" "nucleus" {
  name        = "${var.name_prefix}/nucleus-secret-key"
  description = "Nucleus JWT signing key (optional — empty disables auth)"
}

resource "aws_secretsmanager_secret_version" "nucleus" {
  secret_id     = aws_secretsmanager_secret.nucleus.id
  secret_string = var.nucleus_secret_key
}

resource "aws_secretsmanager_secret" "resend" {
  name        = "${var.name_prefix}/resend-api-key"
  description = "Resend API key for alert email delivery (optional)"
}

resource "aws_secretsmanager_secret_version" "resend" {
  secret_id     = aws_secretsmanager_secret.resend.id
  secret_string = var.resend_api_key
}

resource "aws_secretsmanager_secret" "sentry_dsn" {
  name        = "${var.name_prefix}/sentry-dsn"
  description = "Sentry DSN for error capture (optional)"
}

resource "aws_secretsmanager_secret_version" "sentry_dsn" {
  secret_id     = aws_secretsmanager_secret.sentry_dsn.id
  secret_string = var.sentry_dsn
}
