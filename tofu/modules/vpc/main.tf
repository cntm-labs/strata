# Networking foundation: VPC, public + private subnets across AZs,
# internet gateway, NAT gateway(s), route tables, base security groups.

locals {
  azs       = length(var.azs) > 0 ? var.azs : slice(data.aws_availability_zones.available.names, 0, 2)
  nat_count = var.single_nat_gateway ? 1 : length(local.azs)
}

data "aws_availability_zones" "available" {
  state = "available"
}

resource "aws_vpc" "this" {
  cidr_block           = var.cidr_block
  enable_dns_hostnames = true
  enable_dns_support   = true
  tags                 = { Name = var.name }
}

resource "aws_internet_gateway" "this" {
  vpc_id = aws_vpc.this.id
  tags   = { Name = "${var.name}-igw" }
}

# Public subnets — one per AZ, /24 starting at offset 0.
resource "aws_subnet" "public" {
  count                   = length(local.azs)
  vpc_id                  = aws_vpc.this.id
  cidr_block              = cidrsubnet(var.cidr_block, 8, count.index)
  availability_zone       = local.azs[count.index]
  map_public_ip_on_launch = true
  tags                    = { Name = "${var.name}-public-${local.azs[count.index]}" }
}

# Private subnets — one per AZ, /24 starting at offset 100.
resource "aws_subnet" "private" {
  count             = length(local.azs)
  vpc_id            = aws_vpc.this.id
  cidr_block        = cidrsubnet(var.cidr_block, 8, count.index + 100)
  availability_zone = local.azs[count.index]
  tags              = { Name = "${var.name}-private-${local.azs[count.index]}" }
}

# NAT gateways — one per AZ unless single_nat_gateway is set.
resource "aws_eip" "nat" {
  count  = local.nat_count
  domain = "vpc"
  tags   = { Name = "${var.name}-nat-eip-${count.index}" }
}

resource "aws_nat_gateway" "this" {
  count         = local.nat_count
  allocation_id = aws_eip.nat[count.index].id
  subnet_id     = aws_subnet.public[count.index].id
  tags          = { Name = "${var.name}-nat-${count.index}" }
  depends_on    = [aws_internet_gateway.this]
}

# Public route table — one shared, default route via IGW.
resource "aws_route_table" "public" {
  vpc_id = aws_vpc.this.id
  route {
    cidr_block = "0.0.0.0/0"
    gateway_id = aws_internet_gateway.this.id
  }
  tags = { Name = "${var.name}-public" }
}

resource "aws_route_table_association" "public" {
  count          = length(aws_subnet.public)
  subnet_id      = aws_subnet.public[count.index].id
  route_table_id = aws_route_table.public.id
}

# Private route tables — one per AZ, default route via that AZ's NAT
# (or the single shared NAT if single_nat_gateway is true).
resource "aws_route_table" "private" {
  count  = length(local.azs)
  vpc_id = aws_vpc.this.id
  route {
    cidr_block     = "0.0.0.0/0"
    nat_gateway_id = aws_nat_gateway.this[var.single_nat_gateway ? 0 : count.index].id
  }
  tags = { Name = "${var.name}-private-${local.azs[count.index]}" }
}

resource "aws_route_table_association" "private" {
  count          = length(aws_subnet.private)
  subnet_id      = aws_subnet.private[count.index].id
  route_table_id = aws_route_table.private[count.index].id
}

# Base security groups — empty rules; consuming modules add ingress/egress
# as inline rules referencing these IDs.
resource "aws_security_group" "alb" {
  name        = "${var.name}-alb"
  description = "ALB ingress from internet, egress to ECS tasks"
  vpc_id      = aws_vpc.this.id
  tags        = { Name = "${var.name}-alb" }
}

resource "aws_security_group" "ecs" {
  name        = "${var.name}-ecs"
  description = "ECS tasks: ingress from ALB, egress to RDS + internet"
  vpc_id      = aws_vpc.this.id
  tags        = { Name = "${var.name}-ecs" }
}

resource "aws_security_group" "rds" {
  name        = "${var.name}-rds"
  description = "RDS: ingress from ECS only"
  vpc_id      = aws_vpc.this.id
  tags        = { Name = "${var.name}-rds" }
}

# ALB: ingress 80/443 from internet.
resource "aws_security_group_rule" "alb_ingress_http" {
  security_group_id = aws_security_group.alb.id
  type              = "ingress"
  from_port         = 80
  to_port           = 80
  protocol          = "tcp"
  cidr_blocks       = ["0.0.0.0/0"]
}

resource "aws_security_group_rule" "alb_ingress_https" {
  security_group_id = aws_security_group.alb.id
  type              = "ingress"
  from_port         = 443
  to_port           = 443
  protocol          = "tcp"
  cidr_blocks       = ["0.0.0.0/0"]
}

resource "aws_security_group_rule" "alb_egress_to_ecs" {
  security_group_id        = aws_security_group.alb.id
  type                     = "egress"
  from_port                = 3000
  to_port                  = 3000
  protocol                 = "tcp"
  source_security_group_id = aws_security_group.ecs.id
}

# ECS: ingress from ALB on 3000, egress to RDS 5432 + internet (NAT) 443.
resource "aws_security_group_rule" "ecs_ingress_from_alb" {
  security_group_id        = aws_security_group.ecs.id
  type                     = "ingress"
  from_port                = 3000
  to_port                  = 3000
  protocol                 = "tcp"
  source_security_group_id = aws_security_group.alb.id
}

resource "aws_security_group_rule" "ecs_egress_to_rds" {
  security_group_id        = aws_security_group.ecs.id
  type                     = "egress"
  from_port                = 5432
  to_port                  = 5432
  protocol                 = "tcp"
  source_security_group_id = aws_security_group.rds.id
}

resource "aws_security_group_rule" "ecs_egress_https" {
  security_group_id = aws_security_group.ecs.id
  type              = "egress"
  from_port         = 443
  to_port           = 443
  protocol          = "tcp"
  cidr_blocks       = ["0.0.0.0/0"]
}

# RDS: ingress from ECS on 5432.
resource "aws_security_group_rule" "rds_ingress_from_ecs" {
  security_group_id        = aws_security_group.rds.id
  type                     = "ingress"
  from_port                = 5432
  to_port                  = 5432
  protocol                 = "tcp"
  source_security_group_id = aws_security_group.ecs.id
}
