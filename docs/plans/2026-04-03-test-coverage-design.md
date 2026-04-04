# Test Coverage Design — Strata (0% → 100%)

**Date:** 2026-04-03
**Branch:** feat/full-implementation
**Approach:** Hybrid — Integration tests (DB handlers) + Unit tests (pure logic) + Component tests (Vue)

## Backend (Rust)

### Infrastructure
- `[dev-dependencies]`: axum-test, wiremock, tokio (test), serde_json
- `sqlx::test` with migration fixtures for integration tests
- SQL migrations in `resource/migrations/` for schema setup

### Unit Tests (no DB)

| Module | Tests |
|--------|-------|
| `error/mod.rs` | All 5 AppError variants → correct StatusCode + JSON body; `From<serde_json::Error>` |
| `config/mod.rs` | Defaults without env vars; custom env values; PORT parse fallback |
| `datasource/prometheus.rs` | URL construction, query params, trailing slash trim (wiremock) |
| `datasource/loki.rs` | URL construction, query/query_range params (wiremock) |

### Integration Tests (real PostgreSQL via sqlx::test)

| Module | Tests |
|--------|-------|
| `api/dashboards.rs` | CRUD (5 endpoints) + toggle_star; not-found cases |
| `api/datasources.rs` | CRUD + test_connection (mock external); not-found |
| `api/panels.rs` | list by dashboard, create, update, delete; dashboard not-found |
| `api/alerts.rs` | CRUD rules + list events with/without rule_id |
| `api/templates.rs` | list; use template → creates dashboard + panels |
| `api/explore.rs` | query 3 datasource types (mock external); history; labels; unsupported type |
| `api/query.rs` | proxy_query 3 types + unsupported type |
| `main.rs` | health endpoint → `{"status":"ok"}` |

## Frontend (Vue/TypeScript)

### Infrastructure
- vitest + @vue/test-utils + jsdom
- vitest.config.ts — environment: jsdom, coverage: v8
- Global fetch mock via vi.fn()

### Unit Tests

| Module | Tests |
|--------|-------|
| `api/client.ts` | Success JSON parse; HTTP error throw; fallback message; all 4 methods |
| `api/dashboards.ts` | All functions call correct path + body |
| `api/datasources.ts` | All functions call correct path + body |
| `api/alerts.ts` | listEvents query string building |
| `api/explore.ts` | Query string building; labels path |
| `api/templates.ts` | list + use paths |
| `stores/dashboards.ts` | fetchAll sets items + loading; error resets loading |
| `stores/datasources.ts` | Same pattern |
| `composables/usePanelData.ts` | rangeMap calc; fetchData API call; interval setup/cleanup; no datasource skip |
| `router/index.ts` | All routes resolve; redirect / → /dashboards |

### Component Tests (shallow mount)

| Component | Tests |
|-----------|-------|
| `PanelRenderer.vue` | Renders correct panel by type (8 types) |
| `panels/*.vue` (8) | Props, emits, render without crash |
| `PanelEditor.vue` | Form binding, save emit |
| `views/*.vue` (14) | Mount, correct store/API calls, key elements |
| `layouts/AppLayout.vue` | router-view + sidebar |
| `layouts/AppSidebar.vue` | Nav links, active state |

### Not Tested (covered by build/type-check)
- `main.ts` — app bootstrap
- `types/index.ts` — type-only
- `MonacoEditor.vue` — stubbed in component tests
