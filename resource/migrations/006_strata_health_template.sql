-- 006_strata_health_template.sql
-- Seeds the strata-health self-monitoring dashboard template.
-- Idempotent: ON CONFLICT DO UPDATE keeps the row in sync if the migration
-- is re-applied (which sqlx won't do, but template content may be edited
-- and re-shipped via a follow-up migration).

INSERT INTO dashboard_templates (
    slug,
    name,
    description,
    category,
    dashboard_json,
    required_datasource_type,
    is_active
)
VALUES (
    'strata-health',
    'Strata — self-monitoring',
    'Built-in dashboard scraping Strata''s own /metrics endpoint. Requires a Prometheus datasource pointed at the Strata instance.',
    'observability',
    $json$
{
  "panels": [
    {
      "title": "Requests / sec",
      "type": "stat",
      "query": "sum(rate(strata_http_requests_total[1m]))",
      "position": {"x": 0, "y": 0, "w": 3, "h": 3},
      "config": {"unit": "req/s"}
    },
    {
      "title": "Error rate (%)",
      "type": "stat",
      "query": "100 * sum(rate(strata_http_requests_total{status=~\"5..\"}[1m])) / sum(rate(strata_http_requests_total[1m]))",
      "position": {"x": 3, "y": 0, "w": 3, "h": 3},
      "config": {"unit": "percent"}
    },
    {
      "title": "P95 latency (ms)",
      "type": "stat",
      "query": "histogram_quantile(0.95, sum(rate(strata_http_request_duration_seconds_bucket[5m])) by (le)) * 1000",
      "position": {"x": 6, "y": 0, "w": 3, "h": 3},
      "config": {"unit": "ms"}
    },
    {
      "title": "Active DB connections",
      "type": "gauge",
      "query": "strata_active_connections",
      "position": {"x": 9, "y": 0, "w": 3, "h": 3},
      "config": {"min": 0, "max": 20}
    },
    {
      "title": "Requests by path",
      "type": "timeseries",
      "query": "sum(rate(strata_http_requests_total[1m])) by (path)",
      "position": {"x": 0, "y": 3, "w": 6, "h": 4},
      "config": {}
    },
    {
      "title": "P95 query-proxy latency by datasource",
      "type": "timeseries",
      "query": "histogram_quantile(0.95, sum(rate(strata_query_proxy_duration_seconds_bucket[5m])) by (le, datasource_type))",
      "position": {"x": 6, "y": 3, "w": 6, "h": 4},
      "config": {}
    },
    {
      "title": "Alerts fired (1h)",
      "type": "stat",
      "query": "sum(increase(strata_alerts_fired_total[1h])) by (severity)",
      "position": {"x": 0, "y": 7, "w": 6, "h": 3},
      "config": {}
    },
    {
      "title": "Emails sent (1h)",
      "type": "stat",
      "query": "sum(increase(strata_email_sent_total[1h])) by (status)",
      "position": {"x": 6, "y": 7, "w": 6, "h": 3},
      "config": {}
    }
  ]
}
$json$,
    'prometheus',
    true
)
ON CONFLICT (slug) DO UPDATE SET
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    category = EXCLUDED.category,
    dashboard_json = EXCLUDED.dashboard_json,
    required_datasource_type = EXCLUDED.required_datasource_type,
    is_active = EXCLUDED.is_active;
