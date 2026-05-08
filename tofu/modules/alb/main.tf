# Application Load Balancer with HTTPS termination + HTTP → HTTPS
# redirect. ACM certificate ARN is a required input — operator
# provisions the certificate in the same region before apply.

resource "aws_lb" "this" {
  name               = "${var.name}-alb"
  load_balancer_type = "application"
  internal           = false

  subnets         = var.subnet_ids
  security_groups = var.security_group_ids

  drop_invalid_header_fields = true
  enable_deletion_protection = false

  tags = { Name = "${var.name}-alb" }
}

resource "aws_lb_target_group" "this" {
  name        = "${var.name}-tg"
  vpc_id      = var.vpc_id
  port        = 3000
  protocol    = "HTTP"
  target_type = "ip"

  deregistration_delay = 30

  health_check {
    enabled             = true
    path                = var.health_check_path
    matcher             = "200"
    interval            = 30
    timeout             = 5
    healthy_threshold   = 2
    unhealthy_threshold = 3
    protocol            = "HTTP"
    port                = "traffic-port"
  }

  tags = { Name = "${var.name}-tg" }
}

resource "aws_lb_listener" "https" {
  load_balancer_arn = aws_lb.this.arn
  port              = 443
  protocol          = "HTTPS"
  ssl_policy        = "ELBSecurityPolicy-TLS13-1-2-2021-06"
  certificate_arn   = var.acm_certificate_arn

  default_action {
    type             = "forward"
    target_group_arn = aws_lb_target_group.this.arn
  }
}

resource "aws_lb_listener" "http_redirect" {
  load_balancer_arn = aws_lb.this.arn
  port              = 80
  protocol          = "HTTP"

  default_action {
    type = "redirect"
    redirect {
      protocol    = "HTTPS"
      port        = "443"
      status_code = "HTTP_301"
    }
  }
}
