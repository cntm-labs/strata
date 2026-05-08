# CloudWatch alarms (5xx rate, ECS task count) + SNS topic with email
# subscription + dashboard rendering ALB/ECS/RDS metrics.

resource "aws_sns_topic" "alarms" {
  name = "${var.name}-alarms"
  tags = { Name = "${var.name}-alarms" }
}

resource "aws_sns_topic_subscription" "email" {
  topic_arn = aws_sns_topic.alarms.arn
  protocol  = "email"
  endpoint  = var.alarm_email
}

# 5xx rate alarm — fires when 5xx responses exceed 1% of requests over
# a 5-minute window.
resource "aws_cloudwatch_metric_alarm" "five_xx_rate" {
  alarm_name          = "${var.name}-5xx-rate"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = 1
  threshold           = 0.01
  alarm_description   = "ALB target 5xx rate > 1% over 5 minutes"
  treat_missing_data  = "notBreaching"

  metric_query {
    id          = "rate"
    expression  = "errors / requests"
    label       = "5xx rate"
    return_data = true
  }

  metric_query {
    id = "errors"
    metric {
      metric_name = "HTTPCode_Target_5XX_Count"
      namespace   = "AWS/ApplicationELB"
      period      = 300
      stat        = "Sum"
      dimensions  = { LoadBalancer = var.alb_arn_suffix }
    }
  }

  metric_query {
    id = "requests"
    metric {
      metric_name = "RequestCount"
      namespace   = "AWS/ApplicationELB"
      period      = 300
      stat        = "Sum"
      dimensions  = { LoadBalancer = var.alb_arn_suffix }
    }
  }

  alarm_actions = [aws_sns_topic.alarms.arn]
  ok_actions    = [aws_sns_topic.alarms.arn]
}

# Task count alarm — fires when running tasks fall below desired_count
# for 10 minutes.
resource "aws_cloudwatch_metric_alarm" "task_count" {
  alarm_name          = "${var.name}-task-count-low"
  comparison_operator = "LessThanThreshold"
  evaluation_periods  = 2
  threshold           = var.desired_count
  alarm_description   = "ECS running tasks < desired_count for 10 minutes"
  treat_missing_data  = "breaching"

  metric_name = "RunningTaskCount"
  namespace   = "ECS/ContainerInsights"
  period      = 300
  statistic   = "Average"
  dimensions = {
    ClusterName = var.ecs_cluster_name
    ServiceName = var.ecs_service_name
  }

  alarm_actions = [aws_sns_topic.alarms.arn]
  ok_actions    = [aws_sns_topic.alarms.arn]
}

# Dashboard.
resource "aws_cloudwatch_dashboard" "this" {
  dashboard_name = "${var.name}-dashboard"
  dashboard_body = templatefile("${path.module}/dashboard.json.tpl", {
    region           = var.region
    alb_arn_suffix   = var.alb_arn_suffix
    ecs_cluster_name = var.ecs_cluster_name
    ecs_service_name = var.ecs_service_name
    log_group_name   = var.log_group_name
  })
}
