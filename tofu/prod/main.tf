provider "aws" {
  region = var.region
}

module "vpc" {
  source = "../modules/vpc"
  name   = "strata-prod"
}

module "database" {
  source    = "../modules/rds"
  vpc_id    = module.vpc.vpc_id
  subnets   = module.vpc.private_subnets
  db_name   = "strata"
}

module "app" {
  source          = "../modules/ecs"
  vpc_id          = module.vpc.vpc_id
  subnets         = module.vpc.private_subnets
  db_url          = module.database.connection_url
}
