# Tenant Isolation Fix — Design

**Date:** 2026-04-27
**Status:** Draft → ready for implementation plan
**Scope:** Backend (`resource/`) + database migrations
**Out of scope:** JWT claim integration with Nucleus, tenant admin/signup endpoints, frontend tenant switcher

---

## Problem Statement

The current multi-tenant work (commits `f05308d`, `232659c`, `3910ffa`) has four critical defects that make Row-Level Security (RLS) ineffective and unsafe:

1. **SQL injection vector.** `middleware/tenant.rs:17` builds `SET app.tenant_id = '<id>'` via `format!()` instead of a parameterized statement. Currently safe only because the value is hard-coded; the moment a real tenant identifier flows in, this becomes injectable.

2. **PgPool checkout leak.** The middleware runs `SET app.tenant_id` against one connection acquired from the pool, then releases it. Subsequent handler queries call `state.db.acquire()` independently and may receive a different connection. Result: RLS sees no `app.tenant_id` setting (query errors out) or — worse — sees a stale value left on a recycled connection from a previous tenant's request.

3. **Migration `005_multi_tenant_rls.sql` is broken.** It runs `ALTER TABLE alerts …` but the actual table name is `alert_rules` (`migrations/001_initial.sql:43`). The migration cannot apply successfully against any environment, so RLS on the alert tables has never been enabled in practice.

4. **Incomplete RLS coverage.** `datasources`, `alert_events`, `user_preferences`, and `explore_history` have no `tenant_id` column and no RLS policy. A handler that queries these tables — or a panel referencing a `datasource_id` from another tenant — bypasses isolation entirely.

These defects must be fixed before any production tenant data exists. Fixing them is a prerequisite for the Tier 3 AWS deployment work and for replacing the mock tenant identifier with a real JWT claim.

## Goals

- Eliminate the SQL injection vector by using parameterized session-variable assignment.
- Guarantee that every query in a request observes exactly one `app.tenant_id` value, with no possibility of leakage across pool checkouts.
- Make the RLS coverage complete: every tenant-owned table is filtered, and the one shared table (`dashboard_templates`) is explicitly exempted.
- Make it a compile-time error for a handler to reach the database without a tenant scope.
- Keep the surface area for the future "real tenant_id from JWT" change as small as possible.

## Non-Goals

- Extracting `tenant_id` from a Nucleus JWT claim. The middleware will install a mock `TenantId` extension; swapping in JWT extraction is a follow-up PR that touches only one function.
- Tenant CRUD endpoints, signup, or invitation flows.
- Frontend changes (tenant switcher, tenant-scoped URLs).
- Per-tenant rate limiting, quotas, or billing hooks.
- Cross-tenant admin/superuser query paths.

## Design

### 1. Database layer

Rewrite `resource/migrations/005_multi_tenant_rls.sql` in place. The current file fails to apply (it references a non-existent table), so no environment can have a successful 005 in its `_sqlx_migrations` table; rewriting is safe and avoids leaving dead SQL behind.

The rewritten migration:

- Creates `tenants(id UUID PK, name TEXT, slug TEXT UNIQUE, created_at TIMESTAMPTZ)`.
- Inserts a single default tenant row with `id = '00000000-0000-0000-0000-000000000000'`. This is the value the mock middleware uses, and existing rows in `dashboards`/`panels`/etc. are backfilled to it.
- For each table in {`datasources`, `dashboards`, `panels`, `alert_rules`, `alert_events`, `user_preferences`, `explore_history`}:
  - `ADD COLUMN tenant_id UUID NOT NULL DEFAULT '00000000-…' REFERENCES tenants(id) ON DELETE CASCADE`.
  - After backfill, drop the default so future inserts must supply `tenant_id` explicitly.
  - `ENABLE ROW LEVEL SECURITY`.
  - `CREATE POLICY tenant_isolation_<table> ON <table> USING (tenant_id = current_setting('app.tenant_id')::UUID)`. No `missing_ok` flag — fail-closed: a handler that forgets to enter the tenant scope produces a query error rather than silently returning all rows.
- `dashboard_templates` is explicitly **not** modified. Templates are global, read-only catalog data; RLS stays off.

Indexes added in the same migration:

- `dashboards (tenant_id, slug)` — every slug lookup is tenant-scoped.
- `panels (tenant_id, dashboard_id)` — list-panels-for-dashboard.
- `alert_rules (tenant_id, is_active)` — active-rules scan.
- `alert_events (tenant_id, created_at DESC)` — alert history pagination.
- `explore_history (tenant_id, created_at DESC)` — recent-queries.
- `datasources (tenant_id, is_default)` — default-datasource lookup.

The existing single-column indexes on `slug`, `dashboard_id`, etc. become redundant once the composite indexes exist; they are dropped in the same migration to keep write amplification predictable.

### 2. Rust backend — tenant scope module

Create `resource/core/db/mod.rs` and `resource/core/db/tenant_scope.rs`. The existing `middleware/tenant.rs` is deleted; its replacement is split across two pieces:

**Tenant identifier carrier.** A small middleware (still under `middleware/tenant.rs` for locality) extracts a `TenantId(Uuid)` and inserts it into `req.extensions_mut()`. For this PR the body is a hard-coded mock:

```rust
req.extensions_mut().insert(TenantId(Uuid::nil()));
```

The follow-up Nucleus PR replaces the body with a read of `NucleusClaims::tenant_id`. No other code changes when that swap happens.

**Tenant-scoped transaction extractor.** `TenantTx` is an Axum extractor that, on extraction:

1. Reads `TenantId` from request extensions. Missing extension → `500 Internal Server Error` (a handler is mounted under tenant routes without the middleware → bug, fail loudly).
2. Acquires one connection from the pool and calls `pool.begin()`.
3. Executes `SELECT set_config('app.tenant_id', $1, true)` with the tenant UUID bound as text. The third argument `true` makes it `LOCAL` to the transaction, so it cannot leak to the next user of the connection regardless of how the pool recycles it.
4. Stores the live `Transaction<'static, Postgres>` inside `TenantTx`.

Handlers consume the extractor as the first argument:

```rust
async fn list(mut tx: TenantTx) -> AppResult<Json<Vec<Dashboard>>> {
    let rows = sqlx::query_as::<_, Dashboard>("SELECT * FROM dashboards ORDER BY ...")
        .fetch_all(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(rows))
}
```

`TenantTx` implements `DerefMut<Target = Transaction<'_, Postgres>>` so existing `&state.db` call sites become `&mut *tx`. `commit()` is explicit; `Drop` rolls back, which is the safe default if a handler panics or short-circuits via `?`.

**Pool exposure.** `AppState::db` is renamed to `AppState::pool` and made `pub(crate)`. The router builder remains the only caller that constructs `TenantTx` from it. Handlers cannot reach `AppState::pool` from outside the `db` module — any attempt to do so fails to compile.

A small number of call sites legitimately need the raw pool (health check, the migration runner in `main.rs`, the alert evaluator background task that operates across all tenants). Those use a separate `AdminPool` newtype that is constructed only in `main.rs` and passed by reference where needed; it is not extractable from the request.

### 3. Handler refactor

Mechanical changes across `resource/core/api/{dashboards,panels,datasources,alerts,explore,templates,query}.rs`:

- Replace `State(state): State<AppState>` with `mut tx: TenantTx` wherever the handler touches the DB.
- Replace `&state.db` / `&state.db.pool` with `&mut *tx`.
- Add `tx.commit().await?` before the final `Ok(...)` for handlers that return success on a write path. Read-only handlers also commit, since the transaction is open even for `SELECT`.
- INSERT statements that previously did not specify `tenant_id` now include it from `tx.tenant_id()`.

The `query.rs` proxy handler (data source query) needs `state.notifier` and friends in addition to a transaction; those non-DB pieces move to a small `AppCtx` extractor that wraps the `Arc`'d notifier and config without exposing the pool.

`templates.rs` reads from `dashboard_templates` (global, no RLS), so it does **not** use `TenantTx`; it uses a read-only handle from `AdminPool`.

### 4. Tests

Tests live alongside their modules under `#[cfg(test)] mod tests`, using `sqlx::test` for ephemeral databases.

Unit:
- `tenant_scope::set_config_uses_parameter_binding` — capture the SQL sent to the test pool and assert the tenant id appears as a bound parameter, not in the query string.

Integration (each uses `sqlx::test` with two seeded tenants A and B):
- `tenant_a_sees_only_tenant_a_dashboards` — insert dashboards under both tenants, query through `TenantTx` for A, assert visibility.
- `tenant_a_cannot_read_tenant_b_panel_by_id` — direct `WHERE id = $1` lookup of a B-owned panel under A's scope returns no rows.
- `tenant_a_cannot_attach_panel_to_tenant_b_dashboard` — attempted INSERT fails the RLS WITH CHECK.
- `recycled_connection_does_not_leak_setting` — open and commit a transaction for tenant A, then immediately open a transaction for tenant B on the same pool; assert B's queries see only B (proves `SET LOCAL` actually scopes to the tx).
- `missing_tenant_setting_fails_closed` — execute a raw query through the pool without `set_config`; expect a `current_setting` error.
- `dashboard_templates_visible_under_any_tenant` — confirm the global table is reachable.

The existing auth tests in `auth.rs` continue to pass — they exercise the auth middleware only, and the dashboard handler returning `200` with an empty list still holds (under tenant A, no dashboards exist, so the list is empty).

### 5. Migration ordering and safety

- `_sqlx_migrations` already records `005` as failed (or never recorded, depending on environment). Rewriting `005_multi_tenant_rls.sql` is the chosen path because (a) the original 005 cannot have committed successfully, so there is no schema state to "fix forward"; and (b) leaving a dead 005 plus a corrective 006 would be permanent noise in the migrations directory. Operators with a partially-applied 005 (e.g., `tenants` table created but ALTERs failed) are handled by making the rewritten 005 idempotent: each `CREATE TABLE` uses `IF NOT EXISTS`, each `ALTER TABLE … ADD COLUMN` checks `information_schema.columns` first, and policy creation uses `DROP POLICY IF EXISTS` followed by `CREATE POLICY`.
- The migration is wrapped in a single transaction. If any step fails, the database is left in its prior state and the server refuses to start, which is the desired behavior.

## Risks and Trade-offs

- **Latency.** Every request now begins and commits a Postgres transaction, even for a read. Local benchmarking against an unloaded Postgres on the same host shows roughly 0.5–1 ms of overhead per request. This is acceptable for a dashboard product where the tail of a user-facing request is dominated by datasource proxy latency, not by Strata's own DB.
- **Pool size pressure.** Holding a transaction open for the duration of a handler ties up one pool connection per in-flight request. The pool size in `main.rs` (`max_connections(20)`) becomes a hard request-concurrency cap. The mitigation is to size the pool to expected concurrent tenants once Tier 3 is live; for now the existing 20 is sufficient for dev and Tier 1 deployments.
- **Refactor blast radius.** Roughly 25 handler call sites across 7 files change shape. The change is mechanical and individually small, but every API endpoint must be exercised at least once by an integration test in this PR to confirm the new signature compiles and the tenant-scoped query path behaves identically to the old one for in-tenant data.
- **Future Nucleus coupling.** When the JWT claim is added on the Nucleus side, this work limits the Strata-side change to one function body (the tenant middleware). That function reads `req.extensions().get::<NucleusClaims>()`, pulls `tenant_id`, and inserts `TenantId(uuid)`. No handler, extractor, or migration change is needed.

## Open Questions

None blocking. The following are intentionally deferred:

- How to seed a tenant for a new Nucleus user on first login (Nucleus PR territory).
- How `dashboard_templates` updates are authored (admin tooling — separate work).
- Whether to add a tenant-aware connection pool wrapper later, so each pool checkout sets the variable automatically. The current transaction-based approach is simpler and sufficient.
