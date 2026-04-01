# Strata — Open-Source Observability Dashboard Design Document

> **Date:** 2026-04-01
> **Status:** Approved
> **Author:** mrbt + Claude

## Overview

Strata is an open-source, general-purpose observability dashboard — a Grafana alternative with full UI/UX control. Name means "layers of rock" — peeling back the layers of your stack from hardware through application.

**Design reference:** New Relic (clean UX, alert-centric workflow)

**Primary use case:** Monitor cntm-labs services (Chorus, Nucleus, Orbit) but designed as general-purpose for anyone.

## Goals

1. Replace Grafana with a fully controlled, themeable dashboard
2. Support Prometheus (metrics), Loki (logs), PostgreSQL (app data) as data sources
3. New Relic-inspired UX: clean, focused, alert-centric
4. Auth via Nucleus, alerting via Chorus (when ready — TODO for now)
5. Open-source (MIT), self-hostable, Docker-ready

## Tech Stack

| Layer | Tech | Reason |
|-------|------|--------|
| Backend | Rust + Axum | Same ecosystem as Nucleus/Chorus |
| Frontend | Vue 3 + TypeScript + Vite | User preference, fine-grained reactivity |
| UI Components | PrimeVue | 90+ components, best DataTable, MIT |
| Styling | TailwindCSS + DaisyUI | DaisyUI reduces utility class verbosity |
| Charts (general) | Apache ECharts (vue-echarts) | Heatmaps, gauges, bar, pie — free |
| Charts (time series) | uPlot | 10M+ points at 60fps, 35KB — same as Grafana uses |
| Structured logs | AG Grid Community | Virtual scroll 1M+ rows, filtering, MIT |
| Raw log stream | xterm.js | Terminal-grade performance |
| Dashboard layout | vue-grid-layout | Drag-and-drop panel arrangement |
| Database | PostgreSQL | Dashboards, panels, alerts, preferences |
| Auth | Nucleus (OAuth/JWT) | TODO — integrate when ready |
| Alerting | Chorus (SMS/Email) | TODO — integrate when ready |

## Architecture

```
Browser (Vue 3 SPA)
  │
  │  All API calls
  ▼
Strata Resource (Rust/Axum)
  │
  ├── /api/v1/dashboards/*      → PostgreSQL (CRUD)
  ├── /api/v1/datasources/*/query → Proxy to:
  │   ├── Prometheus (PromQL)
  │   ├── Loki (LogQL)
  │   └── PostgreSQL (SQL)
  ├── /api/v1/alerts/*          → PostgreSQL + Chorus (notifications)
  └── /api/v1/auth/*            → Nucleus (OAuth)
```

Backend proxies all datasource queries — browser never talks to Prometheus/Loki directly (security + no CORS issues).

## Project Structure

```
strata/
├── resource/                    # Rust backend
│   ├── core/
│   │   ├── main.rs
│   │   ├── api/                 # REST API handlers
│   │   ├── datasource/          # Query proxies
│   │   ├── auth/                # Nucleus integration
│   │   ├── config/
│   │   ├── error/
│   │   └── middleware/
│   ├── Cargo.toml
│   └── rustfmt.toml
├── dashboard/                   # Vue 3 frontend
│   ├── src/
│   │   ├── views/
│   │   ├── components/
│   │   ├── composables/
│   │   ├── stores/
│   │   ├── types/
│   │   └── api/
│   ├── tsconfig.json
│   ├── vite.config.ts
│   ├── tailwind.config.ts
│   ├── eslint.config.js
│   └── .prettierrc
├── Cargo.toml
├── SITEMAP.md
├── Dockerfile
└── docker-compose.yml
```

## Data Sources

### Supported (Phase 1)

| Source | Protocol | Query Language |
|--------|----------|---------------|
| Prometheus | HTTP API (`/api/v1/query`, `/api/v1/query_range`) | PromQL |
| Loki | HTTP API (`/loki/api/v1/query`, `/loki/api/v1/query_range`) | LogQL |
| PostgreSQL | Direct connection (sqlx) | SQL |

### Datasource Config

```sql
CREATE TABLE datasources (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    type VARCHAR(50) NOT NULL,
    url TEXT NOT NULL,
    credentials_enc TEXT,
    is_default BOOLEAN DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

## Dashboard & Panel System

### Panel Types

| Type | Library | Use Case |
|------|---------|----------|
| timeseries | uPlot | Metrics over time |
| stat | PrimeVue Card | Single big number |
| gauge | ECharts | Percentage indicators |
| table | AG Grid | Structured data, logs |
| bar | ECharts | Comparisons |
| heatmap | ECharts | Activity/error patterns |
| logs | xterm.js | Raw log streaming |
| piechart | ECharts | Distribution |

### Schema

```sql
CREATE TABLE dashboards (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title VARCHAR(255) NOT NULL,
    slug VARCHAR(100) NOT NULL UNIQUE,
    description TEXT,
    layout JSONB NOT NULL DEFAULT '[]',
    time_range VARCHAR(50) DEFAULT '1h',
    refresh_interval INT DEFAULT 0,
    variables JSONB DEFAULT '[]',
    is_starred BOOLEAN DEFAULT false,
    created_by UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE panels (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    dashboard_id UUID NOT NULL REFERENCES dashboards(id) ON DELETE CASCADE,
    title VARCHAR(255) NOT NULL,
    type VARCHAR(50) NOT NULL,
    datasource_id UUID REFERENCES datasources(id),
    query TEXT NOT NULL,
    config JSONB NOT NULL DEFAULT '{}',
    position JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

Layout uses 12-column grid via vue-grid-layout.

## Auth (Nucleus)

Strata delegates auth entirely to Nucleus via OAuth:
- Login redirects to Nucleus
- JWT verification via Nucleus public key
- Strata stores only user preferences (theme, timezone, default dashboard)

```sql
CREATE TABLE user_preferences (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    nucleus_user_id VARCHAR(255) NOT NULL UNIQUE,
    default_dashboard_id UUID REFERENCES dashboards(id),
    theme VARCHAR(20) DEFAULT 'system',
    timezone VARCHAR(50) DEFAULT 'Asia/Bangkok',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

**NOTE:** Auth is TODO — implement when Nucleus OAuth is production-ready.

## Alerting (Chorus)

Alert rules evaluate PromQL expressions on a schedule. When threshold is exceeded, notifications are sent via Chorus.

```sql
CREATE TABLE alert_rules (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    datasource_id UUID NOT NULL REFERENCES datasources(id),
    query TEXT NOT NULL,
    condition VARCHAR(20) NOT NULL,
    threshold DOUBLE PRECISION NOT NULL,
    duration_secs INT NOT NULL DEFAULT 60,
    severity VARCHAR(20) NOT NULL DEFAULT 'warning',
    notification_channels JSONB NOT NULL,
    notification_recipients JSONB NOT NULL,
    chorus_api_key_enc TEXT,
    is_active BOOLEAN NOT NULL DEFAULT true,
    last_evaluated_at TIMESTAMPTZ,
    current_state VARCHAR(20) DEFAULT 'ok',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE alert_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rule_id UUID NOT NULL REFERENCES alert_rules(id),
    state VARCHAR(20) NOT NULL,
    value DOUBLE PRECISION,
    message TEXT,
    notified_via JSONB DEFAULT '[]',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

**NOTE:** Chorus integration is TODO — implement when Chorus server is running.

## Explore Mode

Ad-hoc query interface for debugging. Auto-detects result type:
- PromQL matrix → timeseries (uPlot)
- PromQL vector → table (AG Grid)
- PromQL scalar → stat
- LogQL streams → logs (xterm.js)
- SQL rows → table (AG Grid)

Results can be pinned to a dashboard as a new panel.

## Dashboard Templates

Built-in templates for quick setup:

| Template | Category | Required Datasource |
|----------|----------|-------------------|
| Chorus Overview | cpaas | Prometheus |
| Nucleus Auth | auth | Prometheus |
| Node Exporter | infrastructure | Prometheus |
| PostgreSQL | database | Prometheus |
| Redis | database | Prometheus |
| Custom Blank | — | — |

```sql
CREATE TABLE dashboard_templates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug VARCHAR(100) NOT NULL UNIQUE,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    category VARCHAR(50) NOT NULL,
    thumbnail_url TEXT,
    dashboard_json JSONB NOT NULL,
    required_datasource_type VARCHAR(50),
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

## Pages

See SITEMAP.md for complete route listing.

**Total: 16 frontend pages + 25 API endpoints**

## Deployment

```yaml
services:
  strata:
    image: strata:latest
    ports: ["3000:3000"]
    environment:
      - DATABASE_URL=postgres://strata:secret@postgres:5432/strata
  postgres:
    image: postgres:16
```

Single Docker container: Rust binary serves Vue static build + API.

## Related Projects

- **Chorus** (github.com/cntm-labs/chorus) — CPaaS, alert notification delivery
- **Nucleus** (github.com/cntm-labs/nucleus) — Auth provider
- **Orbit** (github.com/cntm-labs/orbit-api) — Finance API
