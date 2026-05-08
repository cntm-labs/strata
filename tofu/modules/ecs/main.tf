# ECS Fargate cluster + task definition + service. Tasks run in private
# subnets, pull images from GHCR, read secrets from Secrets Manager via
# the task execution role, write logs to CloudWatch.

resource "aws_ecs_cluster" "this" {
  name = "${var.name}-cluster"

  setting {
    name  = "containerInsights"
    value = "enabled"
  }

  tags = { Name = "${var.name}-cluster" }
}

resource "aws_cloudwatch_log_group" "this" {
  name              = "/ecs/${var.name}"
  retention_in_days = 30
  tags              = { Name = "${var.name}-logs" }
}

# IAM: execution role (used by the ECS agent to pull images, read
# secrets, and write logs).
data "aws_iam_policy_document" "ecs_assume" {
  statement {
    actions = ["sts:AssumeRole"]
    principals {
      type        = "Service"
      identifiers = ["ecs-tasks.amazonaws.com"]
    }
  }
}

resource "aws_iam_role" "execution" {
  name               = "${var.name}-execution"
  assume_role_policy = data.aws_iam_policy_document.ecs_assume.json
}

resource "aws_iam_role_policy_attachment" "execution_managed" {
  role       = aws_iam_role.execution.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AmazonECSTaskExecutionRolePolicy"
}

# Inline policy: read the specific Secrets Manager entries this task
# needs. Scoped to the exact ARNs — no broad secretsmanager:* grant.
data "aws_iam_policy_document" "secrets_read" {
  statement {
    actions   = ["secretsmanager:GetSecretValue"]
    resources = var.secret_arns
  }
}

resource "aws_iam_role_policy" "execution_secrets" {
  name   = "secrets-read"
  role   = aws_iam_role.execution.id
  policy = data.aws_iam_policy_document.secrets_read.json
}

# Task role — for the running container. Currently empty; future
# permissions (S3 export, SES, etc.) attach here.
resource "aws_iam_role" "task" {
  name               = "${var.name}-task"
  assume_role_policy = data.aws_iam_policy_document.ecs_assume.json
}

# Task definition.
resource "aws_ecs_task_definition" "this" {
  family                   = var.name
  network_mode             = "awsvpc"
  requires_compatibilities = ["FARGATE"]
  cpu                      = var.cpu
  memory                   = var.memory
  execution_role_arn       = aws_iam_role.execution.arn
  task_role_arn            = aws_iam_role.task.arn

  container_definitions = jsonencode([{
    name      = "strata"
    image     = "${var.image}:${var.image_tag}"
    essential = true

    portMappings = [{
      containerPort = 3000
      protocol      = "tcp"
    }]

    environment = [
      { name = "HOST", value = "0.0.0.0" },
      { name = "PORT", value = "3000" },
      { name = "STRATA_ENV", value = var.strata_env },
    ]

    secrets = [
      { name = "DATABASE_URL_ADMIN", valueFrom = var.secret_arns_by_name.database_url_admin },
      { name = "DATABASE_URL", valueFrom = var.secret_arns_by_name.database_url },
      { name = "STRATA_APP_PASSWORD", valueFrom = var.secret_arns_by_name.strata_app_password },
      { name = "NUCLEUS_SECRET_KEY", valueFrom = var.secret_arns_by_name.nucleus },
      { name = "RESEND_API_KEY", valueFrom = var.secret_arns_by_name.resend },
      { name = "SENTRY_DSN", valueFrom = var.secret_arns_by_name.sentry_dsn },
    ]

    logConfiguration = {
      logDriver = "awslogs"
      options = {
        "awslogs-group"         = aws_cloudwatch_log_group.this.name
        "awslogs-region"        = var.region
        "awslogs-stream-prefix" = "strata"
      }
    }
  }])

  tags = { Name = "${var.name}-task" }
}

# Service.
resource "aws_ecs_service" "this" {
  name            = "${var.name}-service"
  cluster         = aws_ecs_cluster.this.id
  task_definition = aws_ecs_task_definition.this.arn
  launch_type     = "FARGATE"
  desired_count   = var.desired_count

  network_configuration {
    subnets          = var.subnet_ids
    security_groups  = var.security_group_ids
    assign_public_ip = false
  }

  load_balancer {
    target_group_arn = var.target_group_arn
    container_name   = "strata"
    container_port   = 3000
  }

  deployment_controller {
    type = "ECS"
  }

  health_check_grace_period_seconds = 60

  depends_on = [aws_iam_role_policy.execution_secrets]

  tags = { Name = "${var.name}-service" }
}
