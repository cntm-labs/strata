# Strata — Sitemap

> Current routes and pages for the Strata dashboard application.

## Frontend Routes

### Dashboards

| Route | Page | Description |
|-------|------|-------------|
| `/` | Home | Redirect to `/dashboards` |
| `/dashboards` | Dashboard List | List all dashboards (starred first) |
| `/dashboards/:slug` | Dashboard View | View dashboard with panels |
| `/dashboards/:slug/edit` | Dashboard Edit | Edit panels, layout, settings |
| `/dashboards/new` | New Dashboard | Create from scratch or template |

### Explore

| Route | Page | Description |
|-------|------|-------------|
| `/explore` | Explore | Ad-hoc query mode (PromQL, LogQL, SQL) |

### Alerts

| Route | Page | Description |
|-------|------|-------------|
| `/alerts` | Alerts Overview | Active alerts + rule list |
| `/alerts/rules/new` | New Alert Rule | Create alert rule |
| `/alerts/rules/:id` | Edit Alert Rule | Edit existing rule |
| `/alerts/events` | Alert History | Past firing/resolved events |

### Data Sources

| Route | Page | Description |
|-------|------|-------------|
| `/datasources` | Datasource List | Manage connected datasources |
| `/datasources/new` | Add Datasource | Configure Prometheus/Loki/PostgreSQL |
| `/datasources/:id` | Edit Datasource | Edit connection settings |

### Templates

| Route | Page | Description |
|-------|------|-------------|
| `/templates` | Template Gallery | Browse built-in dashboard templates |

### Settings

| Route | Page | Description |
|-------|------|-------------|
| `/settings` | User Settings | Theme, timezone, default dashboard |

## API Routes (Rust backend)

### Health

| Method | Route | Description |
|--------|-------|-------------|
| GET | `/api/v1/health` | Health check |

### Dashboards
| Method | Route | Description |
|--------|-------|-------------|
| GET | `/api/v1/dashboards` | List dashboards |
| POST | `/api/v1/dashboards` | Create dashboard |
| GET | `/api/v1/dashboards/:slug` | Get dashboard |
| PUT | `/api/v1/dashboards/:slug` | Update dashboard |
| DELETE | `/api/v1/dashboards/:slug` | Delete dashboard |
| POST | `/api/v1/dashboards/:slug/star` | Star/unstar |

### Panels
| Method | Route | Description |
|--------|-------|-------------|
| GET | `/api/v1/dashboards/:slug/panels` | List panels |
| POST | `/api/v1/dashboards/:slug/panels` | Add panel |
| PUT | `/api/v1/panels/:id` | Update panel |
| DELETE | `/api/v1/panels/:id` | Delete panel |

### Data Sources
| Method | Route | Description |
|--------|-------|-------------|
| GET | `/api/v1/datasources` | List datasources |
| POST | `/api/v1/datasources` | Add datasource |
| GET | `/api/v1/datasources/:id` | Get datasource |
| PUT | `/api/v1/datasources/:id` | Update datasource |
| DELETE | `/api/v1/datasources/:id` | Delete datasource |
| POST | `/api/v1/datasources/:id/query` | Proxy query |
| POST | `/api/v1/datasources/:id/test` | Test connection |

### Alerts
| Method | Route | Description |
|--------|-------|-------------|
| GET | `/api/v1/alerts/rules` | List alert rules |
| POST | `/api/v1/alerts/rules` | Create rule |
| GET | `/api/v1/alerts/rules/:id` | Get rule |
| PUT | `/api/v1/alerts/rules/:id` | Update rule |
| DELETE | `/api/v1/alerts/rules/:id` | Delete rule |
| POST | `/api/v1/alerts/rules/:id/test` | Test fire alert rule |
| GET | `/api/v1/alerts/events` | Alert history |

### Explore
| Method | Route | Description |
|--------|-------|-------------|
| POST | `/api/v1/explore/query` | Execute ad-hoc query |
| GET | `/api/v1/explore/history` | Recent queries |
| GET | `/api/v1/explore/labels/:datasource_id` | Label values |

### Templates
| Method | Route | Description |
|--------|-------|-------------|
| GET | `/api/v1/templates` | List dashboard templates |
| POST | `/api/v1/templates/:slug/use` | Create dashboard from template |

## Total: 16 frontend pages + 29 API endpoints
