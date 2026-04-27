# Tenant Isolation Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix tenant isolation defects (SQL injection, PgPool checkout leak, broken migration 005, missing tenant_id on 4 tables) by rewriting migration 005 in place and introducing a `TenantTx` per-request transaction extractor.

**Architecture:** Migration 005 is rewritten to add `tenant_id` (NOT NULL, FK CASCADE) and RLS policies to all 7 tenant-owned tables, plus indexes for the new query shape. Backend introduces a `TenantTx` extractor that opens a transaction per request and runs `set_config('app.tenant_id', $1, true)` (`is_local = true`) so the variable is bound to that transaction and cannot leak across pool checkouts. All 7 API handler files refactor from `State(state)` + `state.db` to `mut tx: TenantTx` + `&mut *tx`.

**Tech Stack:** Rust 1.85, Axum 0.8, sqlx 0.8 (PostgreSQL with `runtime-tokio`, `migrate`, `derive`), PostgreSQL 16 RLS, jsonwebtoken 9.

**Spec:** `docs/superpowers/specs/2026-04-27-tenant-isolation-fix-design.md`

**Working directory:** Run all commands from `resource/` unless stated otherwise.

---

## File Structure

**New files:**
- `resource/core/db/mod.rs` — module root, re-exports
- `resource/core/db/tenant_scope.rs` — `TenantId` struct + `TenantTx` extractor

**Modified files:**
- `resource/migrations/005_multi_tenant_rls.sql` — full rewrite
- `resource/core/main.rs` — register `db` module, rename `AppState::db` → `AppState::pool`
- `resource/core/middleware/tenant.rs` — drop the broken `set_tenant_context`; add a small `inject_mock_tenant` middleware that inserts `TenantId` into request extensions
- `resource/core/middleware/mod.rs` — unchanged exports
- `resource/core/api/dashboards.rs` — handler signatures + queries
- `resource/core/api/panels.rs` — handler signatures + queries
- `resource/core/api/datasources.rs` — handler signatures + queries
- `resource/core/api/alerts.rs` — handler signatures + queries (also keeps `State<AppState>` for `notifier`)
- `resource/core/api/explore.rs` — handler signatures + queries
- `resource/core/api/templates.rs` — read of `dashboard_templates` stays on raw pool; write to `dashboards`/`panels` uses `TenantTx`
- `resource/core/api/query.rs` — `proxy_query` uses `TenantTx` for the datasource lookup

**Test changes:** existing handler tests in each `api/*.rs` file are updated for the new tenant-scoped behavior; new integration tests are added inside `tenant_scope.rs`.

---

## Task 1: Rewrite migration 005 (RLS schema)

**Files:**
- Modify: `resource/migrations/005_multi_tenant_rls.sql`

- [ ] **Step 1: Replace the entire contents of `005_multi_tenant_rls.sql`**

```sql
-- 005_multi_tenant_rls.sql
-- Adds the tenant registry, the tenant_id column and RLS policy to every
-- tenant-owned table, and the composite indexes the new query plans need.
-- Idempotent: safe to re-run after a partial earlier attempt.

BEGIN;

-- 1. Tenant registry
CREATE TABLE IF NOT EXISTS tenants (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name        TEXT NOT NULL,
    slug        TEXT NOT NULL UNIQUE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Default tenant used by the mock middleware and for backfill of pre-existing rows.
INSERT INTO tenants (id, name, slug)
VALUES ('00000000-0000-0000-0000-000000000000', 'Default', 'default')
ON CONFLICT (id) DO NOTHING;

-- 2. Add tenant_id (with default for backfill, then drop the default)
DO $$
DECLARE
    t TEXT;
    tables TEXT[] := ARRAY[
        'datasources',
        'dashboards',
        'panels',
        'alert_rules',
        'alert_events',
        'user_preferences',
        'explore_history'
    ];
BEGIN
    FOREACH t IN ARRAY tables LOOP
        IF NOT EXISTS (
            SELECT 1 FROM information_schema.columns
            WHERE table_name = t AND column_name = 'tenant_id'
        ) THEN
            EXECUTE format(
                'ALTER TABLE %I ADD COLUMN tenant_id UUID NOT NULL
                 DEFAULT ''00000000-0000-0000-0000-000000000000''
                 REFERENCES tenants(id) ON DELETE CASCADE',
                t
            );
            EXECUTE format('ALTER TABLE %I ALTER COLUMN tenant_id DROP DEFAULT', t);
        END IF;
    END LOOP;
END $$;

-- 3. Enable RLS and create policies (drop-then-create for idempotency)
DO $$
DECLARE
    t TEXT;
    tables TEXT[] := ARRAY[
        'datasources',
        'dashboards',
        'panels',
        'alert_rules',
        'alert_events',
        'user_preferences',
        'explore_history'
    ];
BEGIN
    FOREACH t IN ARRAY tables LOOP
        EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', t);
        EXECUTE format('DROP POLICY IF EXISTS tenant_isolation_%I ON %I', t, t);
        EXECUTE format(
            'CREATE POLICY tenant_isolation_%I ON %I
             USING (tenant_id = current_setting(''app.tenant_id'')::UUID)
             WITH CHECK (tenant_id = current_setting(''app.tenant_id'')::UUID)',
            t, t
        );
    END LOOP;
END $$;

-- 4. Composite indexes for tenant-prefixed access patterns
CREATE INDEX IF NOT EXISTS idx_dashboards_tenant_slug
    ON dashboards (tenant_id, slug);
CREATE INDEX IF NOT EXISTS idx_panels_tenant_dashboard
    ON panels (tenant_id, dashboard_id);
CREATE INDEX IF NOT EXISTS idx_datasources_tenant_default
    ON datasources (tenant_id, is_default);
CREATE INDEX IF NOT EXISTS idx_alert_rules_tenant_active
    ON alert_rules (tenant_id, is_active);
CREATE INDEX IF NOT EXISTS idx_alert_events_tenant_created
    ON alert_events (tenant_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_explore_history_tenant_created
    ON explore_history (tenant_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_user_preferences_tenant
    ON user_preferences (tenant_id);

-- 5. Drop now-redundant single-column indexes
DROP INDEX IF EXISTS idx_panels_dashboard_id;
DROP INDEX IF EXISTS idx_alert_events_created_at;
DROP INDEX IF EXISTS idx_explore_history_created_at;

COMMIT;
```

- [ ] **Step 2: Verify the migration applies against a fresh database**

Run from `resource/`:

```bash
docker compose -f ../docker-compose.yml up -d postgres
sleep 3
DATABASE_URL=postgres://strata:secret@localhost:5432/strata cargo sqlx database reset -y
```

Expected: all 5 migrations apply, ending with `Applied 005/migrate multi tenant rls`. No errors.

- [ ] **Step 3: Verify RLS is on and policies exist**

```bash
psql postgres://strata:secret@localhost:5432/strata -c "\
SELECT schemaname, tablename, rowsecurity FROM pg_tables \
 WHERE tablename IN ('datasources','dashboards','panels','alert_rules', \
                     'alert_events','user_preferences','explore_history') \
 ORDER BY tablename;"
```

Expected: `rowsecurity = t` for all 7 tables.

```bash
psql postgres://strata:secret@localhost:5432/strata -c \
  "SELECT polname, polrelid::regclass FROM pg_policy \
    WHERE polname LIKE 'tenant_isolation_%' ORDER BY polname;"
```

Expected: 7 rows, one policy per table.

- [ ] **Step 4: Commit**

```bash
git add resource/migrations/005_multi_tenant_rls.sql
git commit -m "db: rewrite migration 005 with full RLS coverage and indexes"
```

---

## Task 2: Add `db` module skeleton and `TenantId`

**Files:**
- Create: `resource/core/db/mod.rs`
- Create: `resource/core/db/tenant_scope.rs`
- Modify: `resource/core/main.rs` (register the module)

- [ ] **Step 1: Create `resource/core/db/mod.rs`**

```rust
pub mod tenant_scope;

pub use tenant_scope::{TenantId, TenantTx};
```

- [ ] **Step 2: Create `resource/core/db/tenant_scope.rs` with `TenantId` only**

```rust
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
pub struct TenantId(pub Uuid);

pub struct TenantTx {
    // populated in Task 3
    _placeholder: (),
}
```

- [ ] **Step 3: Register the module in `resource/core/main.rs`**

In the top-of-file module declarations (currently `pub mod api; pub mod auth; pub mod config; pub mod datasource; pub mod error; pub mod middleware; pub mod notifier;`), add `pub mod db;` so the file lists modules alphabetically:

```rust
pub mod api;
pub mod auth;
pub mod config;
pub mod datasource;
pub mod db;
pub mod error;
pub mod middleware;
pub mod notifier;
```

- [ ] **Step 4: Verify it compiles**

Run from `resource/`:

```bash
cargo check
```

Expected: clean build. The placeholder `TenantTx` is unused but `_placeholder` suppresses the warning.

- [ ] **Step 5: Commit**

```bash
git add resource/core/db/ resource/core/main.rs
git commit -m "backend: add db module skeleton with TenantId carrier"
```

---

## Task 3: Implement `TenantTx` extractor with unit test

**Files:**
- Modify: `resource/core/db/tenant_scope.rs`

- [ ] **Step 1: Replace the file with the full extractor**

```rust
use std::ops::{Deref, DerefMut};

use axum::{
    extract::{FromRequestParts, State},
    http::request::Parts,
    http::StatusCode,
    response::{IntoResponse, Response},
    RequestPartsExt,
};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::AppState;

#[derive(Debug, Clone, Copy)]
pub struct TenantId(pub Uuid);

pub struct TenantTx {
    tenant_id: Uuid,
    tx: Transaction<'static, Postgres>,
}

impl TenantTx {
    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub async fn commit(self) -> Result<(), sqlx::Error> {
        self.tx.commit().await
    }
}

impl Deref for TenantTx {
    type Target = Transaction<'static, Postgres>;
    fn deref(&self) -> &Self::Target {
        &self.tx
    }
}

impl DerefMut for TenantTx {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.tx
    }
}

pub enum TenantTxError {
    MissingTenant,
    Sqlx(sqlx::Error),
}

impl IntoResponse for TenantTxError {
    fn into_response(self) -> Response {
        match self {
            TenantTxError::MissingTenant => {
                tracing::error!("TenantTx extracted on a route without TenantId middleware");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
            TenantTxError::Sqlx(e) => {
                tracing::error!("TenantTx db error: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

impl FromRequestParts<AppState> for TenantTx {
    type Rejection = TenantTxError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let tenant = parts
            .extensions
            .get::<TenantId>()
            .copied()
            .ok_or(TenantTxError::MissingTenant)?;

        let mut tx = state.pool.begin().await.map_err(TenantTxError::Sqlx)?;

        sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
            .bind(tenant.0.to_string())
            .execute(&mut *tx)
            .await
            .map_err(TenantTxError::Sqlx)?;

        // Suppress unused-warning on the State extractor; not used at this layer.
        let _ = parts.extract::<State<AppState>>().await.ok();

        Ok(Self {
            tenant_id: tenant.0,
            tx,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test(migrations = "./migrations")]
    async fn set_config_uses_parameter_binding(pool: sqlx::PgPool) {
        // The injection vector we want to prevent: a UUID-shaped value with embedded SQL.
        // If the implementation used format!() it would break out of the literal.
        let suspicious = "00000000-0000-0000-0000-000000000000', false); DROP TABLE tenants; --";

        // We don't use the extractor here — we exercise the *underlying call*
        // to prove parameter binding rejects/escapes the input.
        let mut tx = pool.begin().await.unwrap();
        let res = sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
            .bind(suspicious)
            .execute(&mut *tx)
            .await;

        // The bind succeeds (Postgres treats it as a literal text value);
        // the table is still present.
        assert!(res.is_ok());
        let still_exists: (bool,) = sqlx::query_as(
            "SELECT EXISTS (SELECT 1 FROM information_schema.tables \
             WHERE table_name = 'tenants')"
        )
        .fetch_one(&mut *tx)
        .await
        .unwrap();
        assert!(still_exists.0, "tenants table must still exist after bind");
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn set_local_does_not_leak_across_transactions(pool: sqlx::PgPool) {
        // Tenant A
        let a = Uuid::new_v4();
        let mut tx_a = pool.begin().await.unwrap();
        sqlx::query("INSERT INTO tenants (id, name, slug) VALUES ($1, $2, $3)")
            .bind(a)
            .bind("A")
            .bind("a")
            .execute(&mut *tx_a)
            .await
            .unwrap();
        sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
            .bind(a.to_string())
            .execute(&mut *tx_a)
            .await
            .unwrap();
        let seen_a: (String,) = sqlx::query_as("SELECT current_setting('app.tenant_id', true)")
            .fetch_one(&mut *tx_a)
            .await
            .unwrap();
        assert_eq!(seen_a.0, a.to_string());
        tx_a.commit().await.unwrap();

        // Same pool, fresh transaction — must NOT see A's setting
        let mut tx_b = pool.begin().await.unwrap();
        let seen_b: (Option<String>,) =
            sqlx::query_as("SELECT NULLIF(current_setting('app.tenant_id', true), '')")
                .fetch_one(&mut *tx_b)
                .await
                .unwrap();
        assert!(
            seen_b.0.is_none(),
            "expected app.tenant_id unset in a fresh tx, got {:?}",
            seen_b.0
        );
    }
}
```

- [ ] **Step 2: Run the unit tests, expect them to fail to compile**

Run from `resource/`:

```bash
cargo test --no-run -p strata-resource db::tenant_scope
```

Expected: compile error — `AppState::pool` does not exist yet (it is still named `db`). This is the signal to proceed to Task 4.

- [ ] **Step 3: Stop here. Do not commit yet — Task 4 fixes the compile error.**

---

## Task 4: Rename `AppState::db` to `AppState::pool` and wire mock middleware

**Files:**
- Modify: `resource/core/main.rs`
- Modify: `resource/core/middleware/tenant.rs`
- Modify: every test in `resource/core/` that constructs `AppState { db: ..., ... }`

- [ ] **Step 1: Rename the `db` field in `main.rs`**

In `AppState`:

```rust
#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::PgPool,
    pub config: AppConfig,
    pub notifier: Arc<notifier::Notifier>,
}
```

Update the construction in `main()` from `db` to `pool`. Update any place in `main.rs` that previously called `state.db` (notably the `sqlx::migrate!` block and the alert evaluator startup, if present) to call `state.pool`.

- [ ] **Step 2: Replace `middleware/tenant.rs` with the mock injector**

```rust
use axum::{extract::Request, middleware::Next, response::Response};
use uuid::Uuid;

use crate::db::TenantId;

const MOCK_TENANT_ID: Uuid = Uuid::from_u128(0);

pub async fn inject_mock_tenant(mut req: Request, next: Next) -> Response {
    req.extensions_mut().insert(TenantId(MOCK_TENANT_ID));
    next.run(req).await
}
```

- [ ] **Step 3: Update the router builder in `main.rs`**

Replace the existing `set_tenant_context` middleware layer with the new injector. The relevant block becomes:

```rust
let protected = Router::new()
    .nest("/api/v1/datasources", api::datasources::datasource_routes())
    .nest("/api/v1/dashboards", api::dashboards::dashboard_routes())
    .nest("/api/v1", api::panels::panel_routes_nested())
    .nest("/api/v1/explore", api::explore::explore_routes())
    .nest("/api/v1/alerts", api::alerts::alert_routes())
    .nest("/api/v1/templates", api::templates::template_routes())
    .layer(axum::middleware::from_fn(middleware::tenant::inject_mock_tenant));
```

`inject_mock_tenant` does not need state, so `from_fn` (without `_with_state`) is correct.

- [ ] **Step 4: Update test setups across `resource/core/`**

Run from `resource/`:

```bash
cargo build 2>&1 | grep -E "^error" | head -20
```

Each error referencing `db: pool` in a struct literal needs `db:` replaced by `pool:`. Files affected (one occurrence each in test modules):

- `resource/core/auth.rs`
- `resource/core/api/dashboards.rs`
- `resource/core/api/panels.rs`
- `resource/core/api/datasources.rs`
- `resource/core/api/alerts.rs`
- `resource/core/api/explore.rs`
- `resource/core/api/query.rs`
- `resource/core/api/templates.rs`

For each, change `db: pool,` to `pool,` (field-init shorthand, since the local binding is named `pool`).

- [ ] **Step 5: Run the new tenant_scope tests**

```bash
cargo test -p strata-resource db::tenant_scope -- --nocapture
```

Expected: both tests pass.

- [ ] **Step 6: Run the full test suite**

```bash
cargo test -p strata-resource
```

Expected: all existing tests still pass. Any failure here is a leftover `state.db` reference; fix and re-run.

- [ ] **Step 7: Commit**

```bash
git add resource/core resource/migrations
git commit -m "backend: add TenantTx extractor and mock-tenant middleware"
```

---

## Task 5: Refactor `dashboards.rs`

**Files:**
- Modify: `resource/core/api/dashboards.rs`

- [ ] **Step 1: Add a failing cross-tenant isolation test**

Inside the existing `#[cfg(test)] mod tests` block (or create one if none exists), append:

```rust
#[sqlx::test(migrations = "./migrations")]
async fn dashboards_isolated_by_tenant(pool: sqlx::PgPool) {
    use uuid::Uuid;
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    sqlx::query("INSERT INTO tenants (id, name, slug) VALUES ($1, $2, $3), ($4, $5, $6)")
        .bind(a).bind("A").bind(format!("a-{}", a))
        .bind(b).bind("B").bind(format!("b-{}", b))
        .execute(&pool).await.unwrap();

    // Insert one dashboard for each tenant by bypassing RLS via session role.
    sqlx::query(
        "INSERT INTO dashboards (title, slug, layout, tenant_id) \
         VALUES ('A-board', 'a-board', '[]'::jsonb, $1), \
                ('B-board', 'b-board', '[]'::jsonb, $2)"
    ).bind(a).bind(b).execute(&pool).await.unwrap();

    // Open a tx scoped to A; it must see only A.
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
        .bind(a.to_string()).execute(&mut *tx).await.unwrap();
    let titles: Vec<String> = sqlx::query_scalar("SELECT title FROM dashboards ORDER BY title")
        .fetch_all(&mut *tx).await.unwrap();
    assert_eq!(titles, vec!["A-board".to_string()]);
}
```

- [ ] **Step 2: Run the test, expect it to pass**

```bash
cargo test -p strata-resource api::dashboards::tests::dashboards_isolated_by_tenant
```

Expected: pass. (The migration already enables RLS; this test confirms the policy is wired correctly before we touch handlers.)

- [ ] **Step 3: Refactor every handler in `dashboards.rs`**

Replace each handler signature and body. Concrete patterns:

`list`:

```rust
async fn list(mut tx: TenantTx) -> AppResult<Json<Vec<Dashboard>>> {
    let rows = sqlx::query_as::<_, Dashboard>(
        "SELECT * FROM dashboards ORDER BY is_starred DESC, updated_at DESC",
    )
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(rows))
}
```

`create` (note the explicit `tenant_id` insert — RLS WITH CHECK requires it):

```rust
async fn create(
    mut tx: TenantTx,
    Json(input): Json<CreateDashboard>,
) -> AppResult<Json<Dashboard>> {
    let tenant_id = tx.tenant_id();
    let row = sqlx::query_as::<_, Dashboard>(
        "INSERT INTO dashboards (tenant_id, title, slug, description, time_range, refresh_interval, variables) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING *",
    )
    .bind(tenant_id)
    .bind(&input.title)
    .bind(&input.slug)
    .bind(&input.description)
    .bind(&input.time_range)
    .bind(&input.refresh_interval)
    .bind(&input.variables)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(row))
}
```

Apply the same shape to `get_one`, `update`, `remove`, `toggle_star`. In all cases:
- Replace `State(state): State<AppState>` with `mut tx: TenantTx`.
- Replace `&state.db` with `&mut *tx`.
- Add `tx.commit().await?` immediately before the `Ok(...)` return.
- For inserts, add `tenant_id` to the column list and bind `tx.tenant_id()`.

Add the import at the top: `use crate::db::TenantTx;`. Remove `use crate::AppState;` if the file no longer references it.

- [ ] **Step 4: Run the dashboards tests**

```bash
cargo test -p strata-resource api::dashboards
```

Expected: all pass, including the new isolation test.

- [ ] **Step 5: Commit**

```bash
git add resource/core/api/dashboards.rs
git commit -m "backend: refactor dashboards handlers to TenantTx"
```

---

## Task 6: Refactor `panels.rs`

**Files:**
- Modify: `resource/core/api/panels.rs`

- [ ] **Step 1: Inspect current handlers**

```bash
grep -n "State(state)\|state\.db" resource/core/api/panels.rs
```

Expected: one `State(state)` per handler and matching `state.db` calls.

- [ ] **Step 2: Refactor each handler**

For each handler (`list`, `create`, `update`, `remove`):
- Replace `State(state): State<AppState>` with `mut tx: TenantTx`.
- Replace `&state.db` with `&mut *tx`.
- For the `INSERT` in `create`, add `tenant_id` to the column list and bind `tx.tenant_id()`.
- Add `tx.commit().await?` before the `Ok(...)` return.

Add `use crate::db::TenantTx;`; remove the `AppState` import if unused.

- [ ] **Step 3: Add an isolation test**

Append inside the test module:

```rust
#[sqlx::test(migrations = "./migrations")]
async fn panels_visible_only_in_owning_tenant(pool: sqlx::PgPool) {
    use uuid::Uuid;
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    sqlx::query("INSERT INTO tenants (id, name, slug) VALUES ($1, 'A', $2), ($3, 'B', $4)")
        .bind(a).bind(format!("a-{}", a))
        .bind(b).bind(format!("b-{}", b))
        .execute(&pool).await.unwrap();
    let dash_a: Uuid = sqlx::query_scalar(
        "INSERT INTO dashboards (title, slug, layout, tenant_id) \
         VALUES ('A','a','[]'::jsonb,$1) RETURNING id"
    ).bind(a).fetch_one(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO panels (dashboard_id, title, type, query, position, tenant_id) \
         VALUES ($1, 'P', 'stat', '', '{}'::jsonb, $2)"
    ).bind(dash_a).bind(a).execute(&pool).await.unwrap();

    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
        .bind(b.to_string()).execute(&mut *tx).await.unwrap();
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM panels")
        .fetch_one(&mut *tx).await.unwrap();
    assert_eq!(count.0, 0, "tenant B must not see tenant A panels");
}
```

- [ ] **Step 4: Run the panels tests**

```bash
cargo test -p strata-resource api::panels
```

Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add resource/core/api/panels.rs
git commit -m "backend: refactor panels handlers to TenantTx"
```

---

## Task 7: Refactor `datasources.rs`

**Files:**
- Modify: `resource/core/api/datasources.rs`

- [ ] **Step 1: Refactor each handler**

For `list`, `create`, `get_one`, `update`, `remove`, `test_connection`:
- Signature: `mut tx: TenantTx` (drop `State<AppState>`).
- Body: replace `&state.db` with `&mut *tx`.
- `create` INSERT must add `tenant_id` column; bind `tx.tenant_id()`.
- Add `tx.commit().await?` before `Ok(...)`.

Add `use crate::db::TenantTx;`; remove unused `AppState` import.

`test_connection` does not write to the DB but it does load the datasource by id — the read uses `&mut *tx`. Commit at the end to release the transaction even though it is read-only.

- [ ] **Step 2: Add an isolation test**

```rust
#[sqlx::test(migrations = "./migrations")]
async fn datasources_isolated_by_tenant(pool: sqlx::PgPool) {
    use uuid::Uuid;
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    sqlx::query("INSERT INTO tenants (id, name, slug) VALUES ($1,'A',$2),($3,'B',$4)")
        .bind(a).bind(format!("a-{}", a))
        .bind(b).bind(format!("b-{}", b))
        .execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO datasources (name, type, url, tenant_id) \
         VALUES ('promA','prometheus','http://a',$1), \
                ('promB','prometheus','http://b',$2)"
    ).bind(a).bind(b).execute(&pool).await.unwrap();

    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
        .bind(a.to_string()).execute(&mut *tx).await.unwrap();
    let names: Vec<String> = sqlx::query_scalar("SELECT name FROM datasources ORDER BY name")
        .fetch_all(&mut *tx).await.unwrap();
    assert_eq!(names, vec!["promA".to_string()]);
}
```

- [ ] **Step 3: Run the datasource tests**

```bash
cargo test -p strata-resource api::datasources
```

Expected: all pass.

- [ ] **Step 4: Commit**

```bash
git add resource/core/api/datasources.rs
git commit -m "backend: refactor datasources handlers to TenantTx"
```

---

## Task 8: Refactor `alerts.rs` (TenantTx + State for notifier)

**Files:**
- Modify: `resource/core/api/alerts.rs`

- [ ] **Step 1: Refactor read/write handlers to use `TenantTx`**

For `list_rules`, `create_rule`, `get_rule`, `update_rule`, `delete_rule`, `list_events`:
- Signature: `mut tx: TenantTx` instead of `State(state)`.
- Body: replace `&state.db` with `&mut *tx`; add `tx.commit().await?` before each `Ok(...)`.
- `create_rule` INSERT must include `tenant_id`; bind `tx.tenant_id()`.
- INSERTs into `alert_events` (look for the row that records a fired alert) must also include `tenant_id`.

- [ ] **Step 2: Refactor `test_fire_rule` (needs both `TenantTx` and `State<AppState>`)**

This handler reads the rule, fires it, and uses `state.notifier` to send the email. Signature:

```rust
async fn test_fire_rule(
    State(state): State<AppState>,
    mut tx: TenantTx,
    Path(id): Path<Uuid>,
) -> AppResult<Json<...>> {
    // ... DB lookups via &mut *tx ...
    state.notifier.send_alert_email(...).await?;
    // ... insert event via &mut *tx ...
    tx.commit().await?;
    Ok(Json(...))
}
```

The order of `State` and `TenantTx` matters: `State` does not consume the request, so it can come first; `TenantTx` runs after and starts the transaction.

- [ ] **Step 3: Add an isolation test**

```rust
#[sqlx::test(migrations = "./migrations")]
async fn alert_rules_isolated_by_tenant(pool: sqlx::PgPool) {
    use uuid::Uuid;
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    sqlx::query("INSERT INTO tenants (id, name, slug) VALUES ($1,'A',$2),($3,'B',$4)")
        .bind(a).bind(format!("a-{}", a))
        .bind(b).bind(format!("b-{}", b))
        .execute(&pool).await.unwrap();
    let ds_a: Uuid = sqlx::query_scalar(
        "INSERT INTO datasources (name, type, url, tenant_id) \
         VALUES ('p','prometheus','http://x',$1) RETURNING id"
    ).bind(a).fetch_one(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO alert_rules (name, datasource_id, query, condition, threshold, \
         notification_channels, notification_recipients, tenant_id) \
         VALUES ('r', $1, 'up', '>', 0, '[]'::jsonb, '[]'::jsonb, $2)"
    ).bind(ds_a).bind(a).execute(&pool).await.unwrap();

    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
        .bind(b.to_string()).execute(&mut *tx).await.unwrap();
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM alert_rules")
        .fetch_one(&mut *tx).await.unwrap();
    assert_eq!(count.0, 0);
}
```

- [ ] **Step 4: Run the alerts tests**

```bash
cargo test -p strata-resource api::alerts
```

Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add resource/core/api/alerts.rs
git commit -m "backend: refactor alerts handlers to TenantTx"
```

---

## Task 9: Refactor `explore.rs`

**Files:**
- Modify: `resource/core/api/explore.rs`

- [ ] **Step 1: Refactor every handler**

Handlers in this file: `query`, `history`, `labels` (or whatever names exist). For each:
- Signature: `mut tx: TenantTx` (drop `State<AppState>`).
- Body: replace `&state.db` with `&mut *tx`.
- The `INSERT INTO explore_history (...)` must add `tenant_id` and bind `tx.tenant_id()`.
- Add `tx.commit().await?` before `Ok(...)`.

The `labels` handler proxies to a datasource client; the only DB hit is the datasource lookup, which moves to `&mut *tx`.

- [ ] **Step 2: Add an isolation test**

```rust
#[sqlx::test(migrations = "./migrations")]
async fn explore_history_isolated_by_tenant(pool: sqlx::PgPool) {
    use uuid::Uuid;
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    sqlx::query("INSERT INTO tenants (id, name, slug) VALUES ($1,'A',$2),($3,'B',$4)")
        .bind(a).bind(format!("a-{}", a))
        .bind(b).bind(format!("b-{}", b))
        .execute(&pool).await.unwrap();
    let ds_a: Uuid = sqlx::query_scalar(
        "INSERT INTO datasources (name, type, url, tenant_id) \
         VALUES ('p','prometheus','http://x',$1) RETURNING id"
    ).bind(a).fetch_one(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO explore_history (datasource_id, query, query_type, tenant_id) \
         VALUES ($1, 'up', 'promql', $2)"
    ).bind(ds_a).bind(a).execute(&pool).await.unwrap();

    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
        .bind(b.to_string()).execute(&mut *tx).await.unwrap();
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM explore_history")
        .fetch_one(&mut *tx).await.unwrap();
    assert_eq!(count.0, 0);
}
```

- [ ] **Step 3: Run the explore tests**

```bash
cargo test -p strata-resource api::explore
```

Expected: all pass.

- [ ] **Step 4: Commit**

```bash
git add resource/core/api/explore.rs
git commit -m "backend: refactor explore handlers to TenantTx"
```

---

## Task 10: Refactor `templates.rs` (mixed: pool for templates, TenantTx for dashboards)

**Files:**
- Modify: `resource/core/api/templates.rs`

- [ ] **Step 1: Refactor the read handlers (`list`, `get_one` over `dashboard_templates`)**

`dashboard_templates` is global (no RLS), so reads can run on the raw pool. Keep the existing `State(state): State<AppState>` signature for these handlers and continue using `&state.pool`. No `tenant_id` filter applies.

- [ ] **Step 2: Refactor the write handler (`use_template`)**

The current handler reads the template, inserts a dashboard, then iterates over `template.dashboard_json["panels"]` and inserts each panel — all against `&state.db`. The new shape preserves that flow but routes the two write paths through `&mut *tx`, adds `tenant_id` to both inserts, and reads the template from the raw pool (the global table has no RLS):

```rust
async fn use_template(
    State(state): State<AppState>,
    mut tx: TenantTx,
    Path(slug): Path<String>,
    Json(input): Json<UseTemplate>,
) -> AppResult<Json<super::dashboards::Dashboard>> {
    let template = sqlx::query_as::<_, DashboardTemplate>(
        "SELECT * FROM dashboard_templates WHERE slug = $1",
    )
    .bind(&slug)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Template not found".into()))?;

    let tenant_id = tx.tenant_id();

    // Dashboard insert — preserve existing column list, prepend tenant_id.
    let dashboard = sqlx::query_as::<_, super::dashboards::Dashboard>(
        "INSERT INTO dashboards (tenant_id, title, slug, layout) \
         VALUES ($1, $2, $3, COALESCE($4, '[]'::jsonb)) RETURNING *",
    )
    .bind(tenant_id)
    .bind(&input.title)
    .bind(&input.slug)
    .bind(template.dashboard_json.get("layout").cloned())
    .fetch_one(&mut *tx)
    .await?;

    // Panel inserts — preserve the existing panel_json field extraction.
    if let Some(panels) = template.dashboard_json.get("panels").and_then(|v| v.as_array()) {
        for panel_json in panels {
            sqlx::query(
                "INSERT INTO panels (tenant_id, dashboard_id, title, type, datasource_id, query, config, position) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            )
            .bind(tenant_id)
            .bind(dashboard.id)
            .bind(panel_json.get("title").and_then(|v| v.as_str()).unwrap_or("Untitled"))
            .bind(panel_json.get("type").and_then(|v| v.as_str()).unwrap_or("stat"))
            .bind(input.datasource_id)
            .bind(panel_json.get("query").and_then(|v| v.as_str()).unwrap_or(""))
            .bind(panel_json.get("config").unwrap_or(&serde_json::json!({})))
            .bind(panel_json.get("position").unwrap_or(&serde_json::json!({"x":0,"y":0,"w":6,"h":3})))
            .execute(&mut *tx)
            .await?;
        }
    }

    tx.commit().await?;
    Ok(Json(dashboard))
}
```

The only structural changes from the current handler: `state.db` → `&mut *tx` on both write sites, `tenant_id` added as the first column on both inserts, and `tx.commit()` before `Ok(...)`.

- [ ] **Step 3: Run the templates tests**

```bash
cargo test -p strata-resource api::templates
```

Expected: all pass. The existing `list_returns_seeded_templates` test continues to work because `dashboard_templates` is unchanged.

- [ ] **Step 4: Commit**

```bash
git add resource/core/api/templates.rs
git commit -m "backend: route template-use through TenantTx"
```

---

## Task 11: Refactor `query.rs` (datasource proxy)

**Files:**
- Modify: `resource/core/api/query.rs`

- [ ] **Step 1: Refactor `proxy_query`**

```rust
pub async fn proxy_query(
    mut tx: TenantTx,
    Path(id): Path<Uuid>,
    Json(input): Json<QueryRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let ds = sqlx::query_as::<_, super::datasources::Datasource>(
        "SELECT * FROM datasources WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Datasource not found".into()))?;

    // Commit the (read-only) transaction before doing the network proxy.
    tx.commit().await?;

    let result = match ds.ds_type.as_str() {
        // ... existing dispatch unchanged ...
    };
    Ok(Json(result))
}
```

The transaction is committed before the outbound HTTP call so we do not hold a Postgres connection during the proxy round trip.

Add `use crate::db::TenantTx;`; remove `AppState` import if unused (datasources construction does not need AppState here).

- [ ] **Step 2: Add a cross-tenant proxy negative test**

```rust
#[sqlx::test(migrations = "./migrations")]
async fn proxy_query_404s_for_other_tenant_datasource(pool: sqlx::PgPool) {
    use uuid::Uuid;
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    sqlx::query("INSERT INTO tenants (id, name, slug) VALUES ($1,'A',$2),($3,'B',$4)")
        .bind(a).bind(format!("a-{}", a))
        .bind(b).bind(format!("b-{}", b))
        .execute(&pool).await.unwrap();
    let ds_a: Uuid = sqlx::query_scalar(
        "INSERT INTO datasources (name, type, url, tenant_id) \
         VALUES ('p','prometheus','http://x',$1) RETURNING id"
    ).bind(a).fetch_one(&pool).await.unwrap();

    // Open a tx scoped to B and look up A's datasource — must be invisible.
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
        .bind(b.to_string()).execute(&mut *tx).await.unwrap();
    let row: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM datasources WHERE id = $1")
        .bind(ds_a).fetch_optional(&mut *tx).await.unwrap();
    assert!(row.is_none(), "tenant B must not see tenant A's datasource");
}
```

- [ ] **Step 3: Run the query tests**

```bash
cargo test -p strata-resource api::query
```

Expected: all pass.

- [ ] **Step 4: Commit**

```bash
git add resource/core/api/query.rs
git commit -m "backend: route proxy_query through TenantTx"
```

---

## Task 12: Verification sweep

**Files:** none modified (verification only)

- [ ] **Step 1: Confirm no handler still references `state.db` or the old field name**

```bash
grep -rn "state\.db\b" resource/core/
```

Expected: zero matches.

```bash
grep -rn "format!(.*SET app.tenant_id" resource/core/
```

Expected: zero matches.

- [ ] **Step 2: Confirm no handler under `api/` reaches into `state.pool` for tenant-owned tables**

```bash
grep -n "state\.pool" resource/core/api/
```

Expected: only matches in `templates.rs` (read of `dashboard_templates`, which is global). Any other match is a regression — fix it.

- [ ] **Step 3: Run lint and full test suite**

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Expected: clippy passes; all tests pass.

- [ ] **Step 4: Boot the server and exercise the API manually**

```bash
docker compose -f ../docker-compose.yml up -d postgres
cargo run -p strata-resource &
sleep 2
curl -s http://localhost:3000/api/v1/health
curl -s http://localhost:3000/api/v1/dashboards
kill %1
```

Expected: `{"status":"ok"}` and `[]` (empty list under the mock tenant).

- [ ] **Step 5: Final commit (if any incidental fixes were made)**

```bash
git status
git add -p
git commit -m "backend: verification sweep — clean lint and integration"
```

(If `git status` is clean, skip this step.)

---

## Done criteria

- Migration 005 applies on a fresh database without errors and enables RLS on all 7 tenant-owned tables.
- `cargo test -p strata-resource` passes, including the per-handler isolation tests added in Tasks 5–11.
- `cargo clippy --workspace --all-targets -- -D warnings` passes.
- `grep -rn "state\.db\b" resource/core/` returns no results.
- `grep -rn "format!(.*SET app.tenant_id" resource/core/` returns no results.
- Booting the server and hitting `/api/v1/dashboards` returns `[]` under the mock tenant.

After this plan is complete, the follow-up Nucleus PR replaces the body of `inject_mock_tenant` with JWT claim extraction; no other change is required.
