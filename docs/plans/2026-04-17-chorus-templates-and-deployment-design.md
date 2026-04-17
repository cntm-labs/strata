# Chorus Dashboard Templates + Deployment Architecture — Design Doc

> **Status:** Approved design. Ready for implementation planning.
>
> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:writing-plans to create implementation plans from this design.

---

## Overview

Three PRs delivering Chorus observability templates and a deployment architecture design:

| PR | Scope | Issues | Type |
|----|-------|--------|------|
| **A** | Chorus Prometheus Template | #3, #4, #5, #6, #7 | Code (migration) |
| **B** | Chorus Loki + Cost Templates | #8, #9 | Code (migration) |
| **C** | Deployment Design Doc | — | Design only |

---

## PR A: Chorus Prometheus Template

**Goal:** Update existing `chorus-overview` seed template with all Chorus Prometheus metrics panels.

**Approach:** New migration `003_update_chorus_template.sql` that UPDATEs the `dashboard_json` of the existing `chorus-overview` template. Does not modify migration 002.

### Chorus Prometheus Metrics (from `chorus-server/src/metrics.rs`)

| Metric | Type | Labels | Source Issue |
|--------|------|--------|--------------|
| `chorus_messages_total` | counter | channel, status, provider | #4 |
| `chorus_messages_processed_total` | counter | channel | #4 |
| `chorus_provider_errors_total` | counter | channel | #5 |
| `chorus_queue_depth` | gauge | — | #6 |
| `chorus_webhook_deliveries_total` | counter | event, status | #7 |

### Dashboard Layout (10 panels, 12-column grid)

```
Row 0 (y=0, h=3): Overview Stats
┌───────────┬───────────┬───────────┬───────────┐
│  Total    │  Success  │  Queue    │  Webhook  │
│  Messages │  Rate %   │  Depth    │  Success% │
│  (stat)   │  (gauge)  │  (stat)   │  (gauge)  │
│  w=3      │  w=3      │  w=3      │  w=3      │
└───────────┴───────────┴───────────┴───────────┘

Row 1 (y=3, h=3): Message Throughput (#4)
┌─────────────────────┬─────────────────────┐
│ Message Rate /sec   │ Rate by Channel     │
│ (timeseries)        │ SMS vs Email        │
│ w=6                 │ (timeseries) w=6    │
└─────────────────────┴─────────────────────┘

Row 2 (y=6, h=3): Provider Health (#5)
┌─────────────────────┬─────────────────────┐
│ Provider Error Rate │ Messages by Provider│
│ (timeseries) w=6    │ (piechart) w=6      │
└─────────────────────┴─────────────────────┘

Row 3 (y=9, h=3): Queue & Webhooks (#6, #7)
┌─────────────────────┬─────────────────────┐
│ Queue Depth         │ Webhook Deliveries  │
│ (timeseries) w=6    │ (timeseries) w=6    │
└─────────────────────┴─────────────────────┘
```

### Panel Definitions

| # | Title | Type | PromQL |
|---|-------|------|--------|
| 1 | Total Messages | stat | `sum(chorus_messages_total)` |
| 2 | Delivery Success Rate | gauge | `sum(rate(chorus_messages_total{status="delivered"}[5m])) / sum(rate(chorus_messages_total[5m])) * 100` |
| 3 | Queue Depth | stat | `chorus_queue_depth` |
| 4 | Webhook Success Rate | gauge | `sum(rate(chorus_webhook_deliveries_total{status="success"}[5m])) / sum(rate(chorus_webhook_deliveries_total[5m])) * 100` |
| 5 | Message Rate | timeseries | `sum(rate(chorus_messages_total[5m]))` |
| 6 | Rate by Channel | timeseries | `sum(rate(chorus_messages_total[5m])) by (channel)` |
| 7 | Provider Error Rate | timeseries | `sum(rate(chorus_provider_errors_total[5m])) by (channel)` |
| 8 | Messages by Provider | piechart | `sum(chorus_messages_total) by (provider)` |
| 9 | Queue Depth Over Time | timeseries | `chorus_queue_depth` |
| 10 | Webhook Deliveries | timeseries | `sum(rate(chorus_webhook_deliveries_total[5m])) by (event)` |

---

## PR B: Chorus Loki + Cost Templates

**Goal:** Add 2 new templates via migration `004_chorus_logs_costs_templates.sql`.

### Template 1: `chorus-logs` (Issue #8)

**Datasource type:** `loki`

| # | Title | Type | LogQL |
|---|-------|------|-------|
| 1 | Error Rate | stat | `sum(rate({app="chorus"} \|= "ERROR" [1m]))` |
| 2 | Warn Rate | stat | `sum(rate({app="chorus"} \|= "WARN" [1m]))` |
| 3 | Total Log Volume | stat | `sum(rate({app="chorus"} [1m]))` |
| 4 | Log Volume by Level | timeseries | `sum(rate({app="chorus"} [1m])) by (level)` |
| 5 | Chorus Logs | logs | `{app="chorus"}` |

Layout: stats row (y=0, h=3) → volume chart (y=3, h=3) → raw logs (y=6, h=6).

### Template 2: `chorus-costs` (Issue #9)

**Datasource type:** `postgresql`

**Queries reference Chorus's database** (user adds Chorus PostgreSQL as datasource in Strata).

| # | Title | Type | SQL |
|---|-------|------|-----|
| 1 | Total Spend | stat | `SELECT SUM(cost_microdollars) / 1000000.0 AS cost FROM messages WHERE created_at >= NOW() - INTERVAL '30 days'` |
| 2 | SMS Cost | stat | `SELECT SUM(cost_microdollars) / 1000000.0 FROM messages WHERE channel = 'sms' AND created_at >= NOW() - INTERVAL '30 days'` |
| 3 | Email Cost | stat | `SELECT SUM(cost_microdollars) / 1000000.0 FROM messages WHERE channel = 'email' AND created_at >= NOW() - INTERVAL '30 days'` |
| 4 | Daily Cost Trend | timeseries | `SELECT DATE(created_at) AS time, SUM(cost_microdollars) / 1000000.0 AS cost FROM messages WHERE created_at >= NOW() - INTERVAL '30 days' GROUP BY DATE(created_at) ORDER BY time` |
| 5 | Cost by Provider | piechart | `SELECT provider, SUM(cost_microdollars) / 1000000.0 AS cost FROM messages GROUP BY provider ORDER BY cost DESC` |
| 6 | Top Accounts | table | `SELECT account_id, SUM(cost_microdollars) / 1000000.0 AS cost, COUNT(*) AS messages FROM messages GROUP BY account_id ORDER BY cost DESC LIMIT 20` |

Layout: stats row (y=0, h=3) → charts row (y=3, h=3) → table (y=6, h=4).

---

## PR C: Deployment Architecture Design Doc

**Type:** Design document only — no code implementation.

### Deployment Tiers

Implementation order revised based on SaaS priority:

```
Tier 1 (exists) → Tier 4 decision → Tier 3 (AWS) → Tier 2 (Helm)
```

#### Tier 1: Docker Compose (exists, add hardening)
- Resource limits, restart policies
- Reverse proxy (Caddy) for TLS
- PostgreSQL backup cron
- Production `.env.production` example

#### Tier 4: Multi-tenant Architecture Decision (design first)
**Must decide before Tier 3 because it affects RDS strategy.**

| Approach | Isolation | Complexity | Cost |
|----------|-----------|------------|------|
| **Shared DB + `tenant_id`** | Row-level (RLS) | Low | Low (1 RDS) |
| **Schema-per-tenant** | Schema-level | Medium | Low (1 RDS) |
| **DB-per-tenant** | Full | High | High (N RDS) |

Design doc presents trade-offs with recommendation. Decision made when implementing.

#### Tier 3: AWS Managed (ECS Fargate)
- ECS Fargate service (Strata container)
- RDS PostgreSQL (managed, based on Tier 4 decision)
- ALB + ACM for TLS
- ECR or GHCR for container images
- CloudWatch Logs + Metrics
- Terraform modules for IaC

#### Tier 2: Kubernetes / Helm (Enterprise self-hosted)
- Helm chart: Strata + PostgreSQL subchart
- HPA based on CPU/requests
- Ingress (nginx/traefik)
- ConfigMap + Secret management
- PVC for PostgreSQL

### Monitoring Stack (cross-cutting, all tiers)

#### Dogfooding (Strata monitors Strata)
- Strata exposes `/metrics` (Prometheus format)
- Metrics: `strata_http_requests_total`, `strata_query_duration_seconds`, `strata_active_connections`, etc.
- Pre-built `strata-health` dashboard template
- Self-scrape via Prometheus in docker-compose

#### External (production alerting)
| Tool | Purpose | Tier |
|------|---------|------|
| CloudWatch | AWS infra metrics (ECS CPU/mem, RDS) | Tier 3+ |
| Datadog | APM traces, distributed tracing | Tier 3+ |
| PagerDuty | On-call alerting from Strata + CloudWatch | All |
| Sentry | Rust panic/error capture | All |

**Goal:** Learn what production monitoring needs from competitors → build those capabilities into Strata.

---

## Decision Log

| Decision | Choice | Rationale |
|----------|--------|-----------|
| PR structure | 3 separate PRs | Separation of concerns |
| Chorus template | Update existing `chorus-overview` | All-in-one dashboard, no template sprawl |
| Cost data source | Direct DB query (PostgreSQL) | Strata supports it already, Chorus has no API |
| Tier order | 4→3→2 | SaaS priority, Tier 4 decision affects Tier 3 infra |
| Monitoring | Dogfooding + External | Learn from competitors during development |
| Codecov target | 95% | External API paths untestable without live endpoints |
