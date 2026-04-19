INSERT INTO dashboard_templates (slug, name, description, category, dashboard_json, required_datasource_type) VALUES
('chorus-logs', 'Chorus Logs', 'Chorus structured log search, volume tracking, and error detection via Loki', 'cpaas',
 '{
    "panels": [
      {
        "title": "Error Rate /s",
        "type": "stat",
        "query": "sum(rate({app=\"chorus\"} |= \"ERROR\" [1m]))",
        "position": {"x": 0, "y": 0, "w": 4, "h": 3}
      },
      {
        "title": "Warn Rate /s",
        "type": "stat",
        "query": "sum(rate({app=\"chorus\"} |= \"WARN\" [1m]))",
        "position": {"x": 4, "y": 0, "w": 4, "h": 3}
      },
      {
        "title": "Total Log Volume /s",
        "type": "stat",
        "query": "sum(rate({app=\"chorus\"} [1m]))",
        "position": {"x": 8, "y": 0, "w": 4, "h": 3}
      },
      {
        "title": "Log Volume by Level",
        "type": "timeseries",
        "query": "sum(rate({app=\"chorus\"} [1m])) by (level)",
        "position": {"x": 0, "y": 3, "w": 12, "h": 3}
      },
      {
        "title": "Chorus Logs",
        "type": "logs",
        "query": "{app=\"chorus\"}",
        "position": {"x": 0, "y": 6, "w": 12, "h": 6}
      }
    ]
  }',
 'loki'),
('chorus-costs', 'Chorus Cost Analytics', 'Message cost breakdown by channel, provider, and account from Chorus PostgreSQL database', 'cpaas',
 '{
    "panels": [
      {
        "title": "Total Spend (30d)",
        "type": "stat",
        "query": "SELECT SUM(cost_microdollars) / 1000000.0 AS cost FROM messages WHERE created_at >= NOW() - INTERVAL ''30 days''",
        "position": {"x": 0, "y": 0, "w": 4, "h": 3}
      },
      {
        "title": "SMS Cost (30d)",
        "type": "stat",
        "query": "SELECT SUM(cost_microdollars) / 1000000.0 AS cost FROM messages WHERE channel = ''sms'' AND created_at >= NOW() - INTERVAL ''30 days''",
        "position": {"x": 4, "y": 0, "w": 4, "h": 3}
      },
      {
        "title": "Email Cost (30d)",
        "type": "stat",
        "query": "SELECT SUM(cost_microdollars) / 1000000.0 AS cost FROM messages WHERE channel = ''email'' AND created_at >= NOW() - INTERVAL ''30 days''",
        "position": {"x": 8, "y": 0, "w": 4, "h": 3}
      },
      {
        "title": "Daily Cost Trend",
        "type": "timeseries",
        "query": "SELECT DATE(created_at) AS time, SUM(cost_microdollars) / 1000000.0 AS cost FROM messages WHERE created_at >= NOW() - INTERVAL ''30 days'' GROUP BY DATE(created_at) ORDER BY time",
        "position": {"x": 0, "y": 3, "w": 6, "h": 3}
      },
      {
        "title": "Cost by Provider",
        "type": "piechart",
        "query": "SELECT provider, SUM(cost_microdollars) / 1000000.0 AS cost FROM messages GROUP BY provider ORDER BY cost DESC",
        "position": {"x": 6, "y": 3, "w": 6, "h": 3}
      },
      {
        "title": "Top Spending Accounts",
        "type": "table",
        "query": "SELECT account_id, SUM(cost_microdollars) / 1000000.0 AS cost, COUNT(*) AS messages FROM messages GROUP BY account_id ORDER BY cost DESC LIMIT 20",
        "position": {"x": 0, "y": 6, "w": 12, "h": 4}
      }
    ]
  }',
 'postgresql');
