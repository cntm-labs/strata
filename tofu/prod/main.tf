# Strata production environment composition.
#
# Apply order:
#   1. tofu init      (downloads providers; uncomment + configure backend first if using S3)
#   2. tofu plan      (review carefully — this provisions ~$175/mo of AWS resources)
#   3. tofu apply
#
# Sensitive variables (NUCLEUS_SECRET_KEY, RESEND_API_KEY, SENTRY_DSN)
# should be passed via TF_VAR_* env vars, not terraform.tfvars files,
# to keep them out of version control. Generated secrets (Postgres
# password, strata_app password) are managed by the secrets module.

terraform {
  required_version = ">= 1.6"

  required_providers {
    aws    = { source = "hashicorp/aws", version = "~> 5.60" }
    random = { source = "hashicorp/random", version = "~> 3.6" }
  }

  # Operator: uncomment + fill before the first `tofu apply`. Until
  # then, state is local. See README for the bootstrap flow.
  # backend "s3" {
  #   bucket         = "your-tofu-state-bucket"
  #   key            = "strata/prod/terraform.tfstate"
  #   region         = "us-east-1"
  #   dynamodb_table = "your-tofu-state-locks"
  #   encrypt        = true
  # }
}

provider "aws" {
  region = var.region
}

locals {
  name = "strata-${var.environment}"
}

module "vpc" {
  source = "../modules/vpc"

  name               = local.name
  cidr_block         = var.vpc_cidr
  single_nat_gateway = var.single_nat_gateway
}

module "secrets" {
  source = "../modules/secrets"

  name_prefix     = local.name
  nucleus_api_key = var.nucleus_api_key
  resend_api_key  = var.resend_api_key
  sentry_dsn      = var.sentry_dsn
}

module "rds" {
  source = "../modules/rds"

  name                       = local.name
  subnet_ids                 = module.vpc.private_subnet_ids
  security_group_ids         = [module.vpc.rds_security_group_id]
  master_password_secret_arn = module.secrets.postgres_password_secret_arn
  multi_az                   = var.multi_az
  instance_class             = var.rds_instance_class

  depends_on = [module.secrets]
}

# DATABASE_URL secrets are composed here (not in the secrets module)
# to break the secrets ↔ rds dependency cycle: secrets module owns
# the random_passwords; RDS reads the postgres password via data
# source from its ARN; once RDS is up, we read both passwords +
# rds.endpoint here and write the composed URLs as their own secrets.

data "aws_secretsmanager_secret_version" "postgres_pw" {
  secret_id  = module.secrets.postgres_password_secret_arn
  depends_on = [module.secrets]
}

data "aws_secretsmanager_secret_version" "strata_app_pw" {
  secret_id  = module.secrets.strata_app_password_secret_arn
  depends_on = [module.secrets]
}

resource "aws_secretsmanager_secret" "database_url_admin" {
  name        = "${local.name}/database-url-admin"
  description = "Admin (migration role) DATABASE_URL"
}

resource "aws_secretsmanager_secret_version" "database_url_admin" {
  secret_id     = aws_secretsmanager_secret.database_url_admin.id
  secret_string = "postgres://strata:${data.aws_secretsmanager_secret_version.postgres_pw.secret_string}@${module.rds.endpoint}/strata"
}

resource "aws_secretsmanager_secret" "database_url" {
  name        = "${local.name}/database-url"
  description = "Runtime (strata_app role) DATABASE_URL"
}

resource "aws_secretsmanager_secret_version" "database_url" {
  secret_id     = aws_secretsmanager_secret.database_url.id
  secret_string = "postgres://strata_app:${data.aws_secretsmanager_secret_version.strata_app_pw.secret_string}@${module.rds.endpoint}/strata"
}

module "alb" {
  source = "../modules/alb"

  name                = local.name
  vpc_id              = module.vpc.vpc_id
  subnet_ids          = module.vpc.public_subnet_ids
  security_group_ids  = [module.vpc.alb_security_group_id]
  acm_certificate_arn = var.acm_certificate_arn
}

module "ecs" {
  source = "../modules/ecs"

  name               = local.name
  region             = var.region
  vpc_id             = module.vpc.vpc_id
  subnet_ids         = module.vpc.private_subnet_ids
  security_group_ids = [module.vpc.ecs_security_group_id]
  target_group_arn   = module.alb.target_group_arn
  image_tag          = var.image_tag
  desired_count      = var.desired_count
  strata_env         = var.environment

  secret_arns = [
    module.secrets.postgres_password_secret_arn,
    module.secrets.strata_app_password_secret_arn,
    aws_secretsmanager_secret.database_url_admin.arn,
    aws_secretsmanager_secret.database_url.arn,
    module.secrets.nucleus_secret_arn,
    module.secrets.resend_secret_arn,
    module.secrets.sentry_dsn_secret_arn,
  ]
  secret_arns_by_name = {
    database_url_admin  = aws_secretsmanager_secret.database_url_admin.arn
    database_url        = aws_secretsmanager_secret.database_url.arn
    strata_app_password = module.secrets.strata_app_password_secret_arn
    nucleus             = module.secrets.nucleus_secret_arn
    resend              = module.secrets.resend_secret_arn
    sentry_dsn          = module.secrets.sentry_dsn_secret_arn
  }

  depends_on = [
    module.rds,
    module.alb,
    module.secrets,
    aws_secretsmanager_secret_version.database_url_admin,
    aws_secretsmanager_secret_version.database_url,
  ]
}

module "monitoring" {
  source = "../modules/monitoring"

  name             = local.name
  region           = var.region
  alarm_email      = var.alarm_email
  alb_arn_suffix   = module.alb.alb_arn_suffix
  ecs_cluster_name = module.ecs.cluster_name
  ecs_service_name = module.ecs.service_name
  log_group_name   = module.ecs.log_group_name
  desired_count    = var.desired_count
}
