UPDATE dashboard_templates
SET
  description = 'Chorus CPaaS: message throughput, provider health, queue depth, webhook delivery',
  dashboard_json = '{
    "panels": [
      {
        "title": "Total Messages",
        "type": "stat",
        "query": "sum(chorus_messages_total)",
        "position": {"x": 0, "y": 0, "w": 3, "h": 3}
      },
      {
        "title": "Delivery Success Rate",
        "type": "gauge",
        "query": "sum(rate(chorus_messages_total{status=\"delivered\"}[5m])) / sum(rate(chorus_messages_total[5m])) * 100",
        "position": {"x": 3, "y": 0, "w": 3, "h": 3}
      },
      {
        "title": "Queue Depth",
        "type": "stat",
        "query": "chorus_queue_depth",
        "position": {"x": 6, "y": 0, "w": 3, "h": 3}
      },
      {
        "title": "Webhook Success Rate",
        "type": "gauge",
        "query": "sum(rate(chorus_webhook_deliveries_total{status=\"success\"}[5m])) / sum(rate(chorus_webhook_deliveries_total[5m])) * 100",
        "position": {"x": 9, "y": 0, "w": 3, "h": 3}
      },
      {
        "title": "Message Rate",
        "type": "timeseries",
        "query": "sum(rate(chorus_messages_total[5m]))",
        "position": {"x": 0, "y": 3, "w": 6, "h": 3}
      },
      {
        "title": "Rate by Channel",
        "type": "timeseries",
        "query": "sum(rate(chorus_messages_total[5m])) by (channel)",
        "position": {"x": 6, "y": 3, "w": 6, "h": 3}
      },
      {
        "title": "Provider Error Rate",
        "type": "timeseries",
        "query": "sum(rate(chorus_provider_errors_total[5m])) by (channel)",
        "position": {"x": 0, "y": 6, "w": 6, "h": 3}
      },
      {
        "title": "Messages by Provider",
        "type": "piechart",
        "query": "sum(chorus_messages_total) by (provider)",
        "position": {"x": 6, "y": 6, "w": 6, "h": 3}
      },
      {
        "title": "Queue Depth Over Time",
        "type": "timeseries",
        "query": "chorus_queue_depth",
        "position": {"x": 0, "y": 9, "w": 6, "h": 3}
      },
      {
        "title": "Webhook Deliveries",
        "type": "timeseries",
        "query": "sum(rate(chorus_webhook_deliveries_total[5m])) by (event)",
        "position": {"x": 6, "y": 9, "w": 6, "h": 3}
      }
    ]
  }'
WHERE slug = 'chorus-overview';
