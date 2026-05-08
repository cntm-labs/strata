{
  "widgets": [
    {
      "type": "metric",
      "width": 12,
      "height": 6,
      "properties": {
        "title": "HTTP requests / sec (ALB)",
        "region": "${region}",
        "metrics": [
          ["AWS/ApplicationELB", "RequestCount", "LoadBalancer", "${alb_arn_suffix}", { "stat": "Sum", "period": 60 }]
        ]
      }
    },
    {
      "type": "metric",
      "width": 12,
      "height": 6,
      "properties": {
        "title": "5xx count (ALB)",
        "region": "${region}",
        "metrics": [
          ["AWS/ApplicationELB", "HTTPCode_Target_5XX_Count", "LoadBalancer", "${alb_arn_suffix}", { "stat": "Sum", "period": 60 }]
        ]
      }
    },
    {
      "type": "metric",
      "width": 12,
      "height": 6,
      "properties": {
        "title": "ECS task count",
        "region": "${region}",
        "metrics": [
          ["ECS/ContainerInsights", "RunningTaskCount", "ClusterName", "${ecs_cluster_name}", "ServiceName", "${ecs_service_name}", { "stat": "Average" }]
        ]
      }
    },
    {
      "type": "metric",
      "width": 12,
      "height": 6,
      "properties": {
        "title": "ECS task CPU + memory",
        "region": "${region}",
        "metrics": [
          ["ECS/ContainerInsights", "CpuUtilized", "ClusterName", "${ecs_cluster_name}", "ServiceName", "${ecs_service_name}"],
          ["ECS/ContainerInsights", "MemoryUtilized", "ClusterName", "${ecs_cluster_name}", "ServiceName", "${ecs_service_name}"]
        ]
      }
    },
    {
      "type": "log",
      "width": 24,
      "height": 6,
      "properties": {
        "title": "Recent ERROR logs",
        "region": "${region}",
        "query": "SOURCE '${log_group_name}' | filter @message like /ERROR/ | sort @timestamp desc | limit 50"
      }
    }
  ]
}
