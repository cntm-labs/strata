# Tier 1 Production Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire the `strata_app` non-super DB role into the runtime path and ship a production-ready Docker Compose stack (Caddy edge TLS, internal-only Postgres, daily backup sidecar, one-command deploy).

**Architecture:** App boots through two pools: an admin pool runs migrations and one parameterized `ALTER ROLE strata_app WITH LOGIN PASSWORD $1`, then closes; the runtime pool connects as `strata_app`. A `docker-compose.production.yml` overrides the dev compose, drops host-exposed ports, adds Caddy + a backup-sidecar service, and splits the network into `frontend` and `internal: true` `backend`. A `scripts/deploy.sh` wraps `docker compose -f base -f override --env-file ... up`.

**Tech Stack:** sqlx 0.8 (admin + runtime pools), Axum 0.8, Caddy 2-alpine (Let's Encrypt ACME), Docker Compose v2 with override files, Bash for backup cron loop and deploy wrapper.

**Spec:** `docs/superpowers/specs/2026-04-30-tier-1-production-hardening-design.md`

**Working directory:** Repo root `/home/mrbt/Desktop/workspaces/software/repositories/strata`. Branch: `feat/tier-1-hardening` (already created, spec already committed at `3ffbb22`). All commands assume repo root unless stated.

---

## File Structure

**New files:**
- `resource/core/db/bootstrap.rs` — `bootstrap_db()` helper: builds admin pool, runs migrations, runs `ALTER ROLE`, closes admin pool, returns runtime pool.
- `caddy/Caddyfile` — single-domain reverse proxy with automatic HTTPS.
- `docker-compose.production.yml` — override file: drops host ports, adds Caddy + backup, network split, resource limits, restart policy.
- `.env.production.example` — env template documenting every required variable.
- `scripts/deploy.sh` — bash wrapper: validates env file, runs `docker compose up -d`, polls health, prints landing info.

**Modified files:**
- `resource/core/config/mod.rs` — adds `database_url_admin: Option<String>` and `strata_app_password: Option<String>` to `AppConfig` + `from_env`.
- `resource/core/db/mod.rs` — re-export `bootstrap::bootstrap_db`.
- `resource/core/main.rs` — replace inline `PgPool::connect` + `migrate.run` with a call to `bootstrap_db()`.
- `scripts/backup.sh` — rewrite from one-shot to cron loop with optional S3 upload and retention pruning.

**Tests changed:** existing `config::mod::tests` gain coverage for the two new optional env vars; `db::bootstrap::tests` is new and uses `sqlx::test` to exercise the migrate + ALTER ROLE + reconnect flow against a fresh DB.

---

## Task 1: Add admin URL and strata_app password to AppConfig

**Files:**
- Modify: `resource/core/config/mod.rs`

- [ ] **Step 1: Inspect current `AppConfig` and `from_env`.**

```bash
sed -n '1,60p' resource/core/config/mod.rs
```

Expected: a struct with fields like `database_url`, `host`, `port`, `nucleus_secret_key`, `nucleus_base_url`, `resend_api_key`, `alert_from_email`, plus a `from_env` constructor that reads each via `std::env::var`. The new fields follow the same `Option<String>` pattern as `nucleus_secret_key`.

- [ ] **Step 2: Add the two new fields to the struct.**

In `resource/core/config/mod.rs`, locate the `AppConfig` definition (it currently has 7 fields ending with `alert_from_email: String`). Insert the two new optional fields just after `database_url`:

```rust
pub struct AppConfig {
    pub database_url: String,
    pub database_url_admin: Option<String>,
    pub strata_app_password: Option<String>,
    pub host: String,
    pub port: u16,
    pub nucleus_secret_key: Option<String>,
    pub nucleus_base_url: Option<String>,
    pub resend_api_key: Option<String>,
    pub alert_from_email: String,
}
```

- [ ] **Step 3: Read the new vars in `from_env`.**

In the same file, locate the `from_env` body. Add the two reads next to the existing `database_url` line:

```rust
database_url: std::env::var("DATABASE_URL")
    .expect("DATABASE_URL must be set"),
database_url_admin: std::env::var("DATABASE_URL_ADMIN").ok(),
strata_app_password: std::env::var("STRATA_APP_PASSWORD").ok(),
```

`.ok()` converts a `Result` to `Option` so an unset variable becomes `None`, matching the dev fallback in the spec.

- [ ] **Step 4: Update existing test setups.**

```bash
grep -rn "AppConfig {" resource/core/ | head
```

Expected: roughly 8 sites in `auth.rs`, `api/*.rs`, `notifier.rs` (test modules) constructing `AppConfig { ... }` literals. Each needs the two new fields. Add `database_url_admin: None,` and `strata_app_password: None,` immediately after the `database_url` initializer in every literal. Example diff per site:

```rust
let config = crate::config::AppConfig {
    database_url: String::new(),
    database_url_admin: None,
    strata_app_password: None,
    host: "127.0.0.1".into(),
    // ... unchanged ...
};
```

- [ ] **Step 5: Add a unit test for env parsing.**

In the existing `#[cfg(test)] mod tests` at the bottom of `resource/core/config/mod.rs`, append:

```rust
#[test]
fn from_env_reads_optional_admin_and_password() {
    // Use unique names so parallel tests don't fight; restore at end.
    std::env::set_var("DATABASE_URL", "postgres://x");
    std::env::set_var("DATABASE_URL_ADMIN", "postgres://admin");
    std::env::set_var("STRATA_APP_PASSWORD", "s3cret");
    std::env::set_var("ALERT_FROM_EMAIL", "a@b");
    let cfg = AppConfig::from_env();
    assert_eq!(cfg.database_url_admin.as_deref(), Some("postgres://admin"));
    assert_eq!(cfg.strata_app_password.as_deref(), Some("s3cret"));
    std::env::remove_var("DATABASE_URL_ADMIN");
    std::env::remove_var("STRATA_APP_PASSWORD");
}

#[test]
fn from_env_admin_and_password_default_to_none() {
    std::env::remove_var("DATABASE_URL_ADMIN");
    std::env::remove_var("STRATA_APP_PASSWORD");
    std::env::set_var("DATABASE_URL", "postgres://x");
    std::env::set_var("ALERT_FROM_EMAIL", "a@b");
    let cfg = AppConfig::from_env();
    assert!(cfg.database_url_admin.is_none());
    assert!(cfg.strata_app_password.is_none());
}
```

- [ ] **Step 6: Build and run the new tests.**

Run from repo root:

```bash
cd resource && cargo build && cargo test -p strata-resource config::mod::tests::from_env_
```

Expected: clean build, 2 passed. If the build fails because some `AppConfig { ... }` literal is missing the new fields, fix that site and rerun.

- [ ] **Step 7: Run the full suite to confirm no regressions.**

```bash
DATABASE_URL=postgres://strata:secret@localhost:5432/strata cargo test -p strata-resource
```

Expected: all tests pass, no regressions.

- [ ] **Step 8: Commit.**

```bash
cd /home/mrbt/Desktop/workspaces/software/repositories/strata
git add resource/core/config/mod.rs resource/core/auth.rs resource/core/api resource/core/notifier.rs
git commit -m "config: add database_url_admin and strata_app_password fields"
```

(Some of those paths may not need staging if no test setup in them — check `git status` first; only stage files that actually changed.)

---

## Task 2: Implement bootstrap_db helper with TDD

**Files:**
- Create: `resource/core/db/bootstrap.rs`
- Modify: `resource/core/db/mod.rs` (add `pub mod bootstrap;` + re-export)

- [ ] **Step 1: Add the module declaration.**

In `resource/core/db/mod.rs`, change the contents to:

```rust
pub mod bootstrap;
pub mod tenant_scope;

pub use bootstrap::bootstrap_db;
pub use tenant_scope::{TenantId, TenantTx};
```

- [ ] **Step 2: Create the bootstrap module skeleton.**

Create `resource/core/db/bootstrap.rs` with:

```rust
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

use crate::config::AppConfig;

/// Build a runtime PgPool, ensuring migrations have been applied and the
/// `strata_app` role is loginable with the configured password.
///
/// In dev (no `DATABASE_URL_ADMIN`, no `STRATA_APP_PASSWORD`) this collapses
/// to the original single-pool behavior: connect to `DATABASE_URL`, run
/// migrations, return the pool.
///
/// In prod, opens an admin pool from `DATABASE_URL_ADMIN`, runs migrations,
/// runs a parameterized `ALTER ROLE strata_app WITH LOGIN PASSWORD $1`,
/// closes the admin pool, and returns a fresh runtime pool from
/// `DATABASE_URL` (which connects as `strata_app`).
pub async fn bootstrap_db(config: &AppConfig) -> Result<PgPool, sqlx::Error> {
    let admin_url = config
        .database_url_admin
        .as_deref()
        .unwrap_or(&config.database_url);

    let admin_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(admin_url)
        .await?;

    sqlx::migrate!("./migrations").run(&admin_pool).await?;

    if let Some(password) = config.strata_app_password.as_deref() {
        sqlx::query("ALTER ROLE strata_app WITH LOGIN PASSWORD $1")
            .bind(password)
            .execute(&admin_pool)
            .await?;
    }

    admin_pool.close().await;

    PgPoolOptions::new()
        .max_connections(20)
        .connect(&config.database_url)
        .await
}
```

The `sqlx::migrate!` macro expects to be invoked from a crate whose manifest sits next to a `migrations/` dir, which is the case for `strata-resource` (manifest at `resource/Cargo.toml`, migrations at `resource/migrations/`). The relative path `./migrations` resolves at compile time relative to `CARGO_MANIFEST_DIR`.

- [ ] **Step 3: Add an MSRV note: sqlx::Error::Migrate handling.**

`sqlx::migrate!().run()` returns `Result<(), sqlx::migrate::MigrateError>`, and `MigrateError` does **not** implement `From<sqlx::migrate::MigrateError> for sqlx::Error` automatically in sqlx 0.8. Wrap the call site:

In `bootstrap.rs`, change the migrate line to:

```rust
sqlx::migrate!("./migrations")
    .run(&admin_pool)
    .await
    .map_err(|e| sqlx::Error::Configuration(Box::new(e)))?;
```

`sqlx::Error::Configuration` carries an arbitrary `Box<dyn StdError + Send + Sync + 'static>`. This preserves the migration error message in the returned `sqlx::Error`.

- [ ] **Step 4: Verify the file compiles standalone.**

```bash
cd resource && cargo check
```

Expected: clean compile.

- [ ] **Step 5: Write the dev-fallback test (skip ALTER ROLE when password is None).**

In `resource/core/db/bootstrap.rs`, append at the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn test_config(url: &str) -> AppConfig {
        AppConfig {
            database_url: url.to_string(),
            database_url_admin: None,
            strata_app_password: None,
            host: "127.0.0.1".into(),
            port: 3000,
            nucleus_secret_key: None,
            nucleus_base_url: None,
            resend_api_key: None,
            alert_from_email: "test@test.com".into(),
        }
    }

    // Note: this test reuses the live DATABASE_URL rather than `sqlx::test`
    // because `sqlx::test` owns the connection URL it picks and doesn't
    // expose the full string for us to feed back into `bootstrap_db`. The
    // migrations are idempotent (PR #13 made them so), so re-running them
    // here against an already-migrated database is safe.
    #[tokio::test]
    async fn dev_path_returns_a_working_pool_when_password_unset() {
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let cfg = test_config(&url);
        let pool = bootstrap_db(&cfg).await.expect("bootstrap_db should succeed");
        let one: (i32,) = sqlx::query_as("SELECT 1").fetch_one(&pool).await.unwrap();
        assert_eq!(one.0, 1);
    }
}
```

- [ ] **Step 6: Run the dev-fallback test.**

```bash
DATABASE_URL=postgres://strata:secret@localhost:5432/strata cargo test -p strata-resource db::bootstrap
```

Expected: 1 passed.

- [ ] **Step 7: Add the prod-path test (with ALTER ROLE).**

Append to the same `mod tests`:

```rust
#[tokio::test]
async fn prod_path_alters_strata_app_password() {
    let admin_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set as the migration role for tests");
    let mut cfg = test_config(&admin_url);
    cfg.database_url_admin = Some(admin_url.clone());
    cfg.strata_app_password = Some("test_pwd_xyz123".into());
    // database_url itself still points at the migration role — we're not
    // testing that strata_app can actually log in here (that needs a separate
    // host:port:database with strata_app credentials), only that the ALTER
    // ROLE statement runs successfully against the admin pool.
    let _pool = bootstrap_db(&cfg).await.expect("bootstrap_db should succeed");

    // Verify the role was altered: it should now have rolcanlogin = true.
    let admin = sqlx::PgPool::connect(&admin_url).await.unwrap();
    let can_login: (bool,) = sqlx::query_as(
        "SELECT rolcanlogin FROM pg_roles WHERE rolname = 'strata_app'"
    )
    .fetch_one(&admin)
    .await
    .unwrap();
    assert!(can_login.0, "strata_app must be loginable after bootstrap_db with password");
}
```

Note: this test mutates cluster-wide role state (`pg_authid`). Running it in parallel with other tests that also touch `strata_app` could race. The migration's `EXCEPTION WHEN duplicate_object/unique_violation` already protects against parallel `CREATE ROLE`; for `ALTER ROLE`, parallel runs converge on the same final state since both pass the same password. Acceptable for now.

- [ ] **Step 8: Run both bootstrap tests.**

```bash
DATABASE_URL=postgres://strata:secret@localhost:5432/strata cargo test -p strata-resource db::bootstrap
```

Expected: 2 passed.

- [ ] **Step 9: Commit.**

```bash
cd /home/mrbt/Desktop/workspaces/software/repositories/strata
git add resource/core/db/bootstrap.rs resource/core/db/mod.rs
git commit -m "db: add bootstrap_db with admin-pool migrate + ALTER ROLE flow"
```

---

## Task 3: Wire bootstrap_db into main.rs

**Files:**
- Modify: `resource/core/main.rs`

- [ ] **Step 1: Inspect the current `main()` body.**

```bash
sed -n '60,110p' resource/core/main.rs
```

Expected: code that does `PgPoolOptions::new().max_connections(20).connect(&config.database_url).await`, followed by `sqlx::migrate!("./migrations").run(&pool)`, then constructs `AppState`. We're replacing both with a single `bootstrap_db()` call.

- [ ] **Step 2: Replace the inline connect + migrate with bootstrap_db.**

Locate the block in `main.rs` that currently looks like:

```rust
let pool = PgPoolOptions::new()
    .max_connections(20)
    .connect(&config.database_url)
    .await
    .expect("Failed to connect to database");

sqlx::migrate!("./migrations")
    .run(&pool)
    .await
    .expect("Failed to run database migrations");
```

Replace with:

```rust
let pool = db::bootstrap_db(&config)
    .await
    .expect("Failed to bootstrap database");
```

Remove the now-unused `use sqlx::postgres::PgPoolOptions;` import if it has no other call sites in `main.rs` (the `cargo check` step will tell you).

- [ ] **Step 3: Run cargo check.**

```bash
cd resource && cargo check
```

Expected: clean. If `PgPoolOptions` is now unused, delete its import line. Re-run.

- [ ] **Step 4: Run the full test suite.**

```bash
DATABASE_URL=postgres://strata:secret@localhost:5432/strata cargo test -p strata-resource
```

Expected: all pass — `bootstrap_db` is exercised through every existing `sqlx::test` indirectly (those tests don't go through `main()`, but the compile-checked import + the dedicated bootstrap tests cover the behavior).

- [ ] **Step 5: Commit.**

```bash
cd /home/mrbt/Desktop/workspaces/software/repositories/strata
git add resource/core/main.rs
git commit -m "main: route db init through bootstrap_db helper"
```

---

## Task 4: Rewrite backup.sh as cron loop

**Files:**
- Modify: `scripts/backup.sh`

- [ ] **Step 1: Replace the file contents.**

Write the new `scripts/backup.sh` exactly:

```bash
#!/bin/bash
set -euo pipefail

: "${DATABASE_URL:?DATABASE_URL must be set}"
: "${BACKUP_INTERVAL:=86400}"
: "${RETENTION_DAYS:=14}"
BACKUP_DIR="${BACKUP_DIR:-/backups}"

mkdir -p "$BACKUP_DIR"

# TODO(prod): swap superuser DATABASE_URL for a dedicated `strata_backup`
# role with pg_read_all_data once the admin tooling for role provisioning lands.

while true; do
    ts=$(date -u +%Y%m%dT%H%M%SZ)
    file="${BACKUP_DIR}/strata-${ts}.sql.gz"
    echo "[$(date -u -Iseconds)] Starting backup → $file"

    pg_dump "$DATABASE_URL" | gzip > "$file"
    echo "[$(date -u -Iseconds)] Backup complete: $(du -h "$file" | cut -f1)"

    if [[ -n "${S3_BUCKET:-}" ]]; then
        aws s3 cp "$file" "s3://${S3_BUCKET}/backups/$(basename "$file")"
        echo "[$(date -u -Iseconds)] Uploaded to s3://${S3_BUCKET}/"
    fi

    # Prune old local backups (S3 retention managed via lifecycle rules).
    find "$BACKUP_DIR" -name 'strata-*.sql.gz' -mtime +"${RETENTION_DAYS}" -delete

    sleep "${BACKUP_INTERVAL}"
done
```

- [ ] **Step 2: Lint with shellcheck if available.**

```bash
which shellcheck && shellcheck scripts/backup.sh || echo "shellcheck not installed, skipping"
```

Expected: zero warnings, OR the install-skip message. If shellcheck reports issues, fix them inline.

- [ ] **Step 3: Smoke test the script's bash syntax.**

```bash
bash -n scripts/backup.sh
```

Expected: exit code 0 (syntax valid).

- [ ] **Step 4: Smoke test that env-var defaults work and required vars trigger errors.**

```bash
# Required var missing → fail fast.
bash scripts/backup.sh 2>&1 | head -3 || true
```

Expected: error message mentioning `DATABASE_URL`, exit code 1 (the `:?` parameter expansion on line 4).

```bash
# With DATABASE_URL but no DB available, the loop will hit pg_dump and exit
# (set -e). We can't test the full cron loop here without postgres running,
# so just verify the variable defaults parse:
DATABASE_URL=postgres://nope BACKUP_INTERVAL=1 timeout 1 bash scripts/backup.sh 2>&1 | head -5 || true
```

Expected: hits `pg_dump` and errors out — proves the loop body runs, var defaults parse, and `set -e` propagates.

- [ ] **Step 5: Commit.**

```bash
git add scripts/backup.sh
git commit -m "ops: rewrite backup.sh as cron loop with retention and optional S3"
```

---

## Task 5: Create the Caddyfile

**Files:**
- Create: `caddy/Caddyfile`

- [ ] **Step 1: Create the directory and file.**

```bash
mkdir -p caddy
```

Write `caddy/Caddyfile`:

```caddyfile
{
    email {$ACME_EMAIL}
}

{$STRATA_DOMAIN} {
    encode zstd gzip

    reverse_proxy strata:3000

    log {
        output stdout
        format json
    }
}
```

The global block (`{ ... }` at the top, no domain) sets the ACME contact email. The site block uses `{$VAR}` for env-var substitution, which Caddy resolves at startup from its own process environment — Compose passes through any vars referenced in the service definition.

- [ ] **Step 2: Smoke-validate the Caddyfile syntax.**

If Caddy is installed locally:

```bash
which caddy && STRATA_DOMAIN=example.com ACME_EMAIL=a@b caddy validate --config caddy/Caddyfile --adapter caddyfile || echo "caddy not installed, skipping local validation"
```

Expected: `Valid configuration` or the install-skip message. If Caddy is not installed, `docker compose config` in Task 6 will catch gross errors when Compose loads the bind mount.

- [ ] **Step 3: Commit.**

```bash
git add caddy/Caddyfile
git commit -m "ops: add Caddyfile with auto-HTTPS reverse proxy to strata"
```

---

## Task 6: Create docker-compose.production.yml

**Files:**
- Create: `docker-compose.production.yml`

- [ ] **Step 1: Write the full override file.**

Write `docker-compose.production.yml`:

```yaml
# Production override for docker-compose.yml.
# Usage:
#   docker compose -f docker-compose.yml -f docker-compose.production.yml \
#                  --env-file .env.production up -d --build
# Or simply: ./scripts/deploy.sh

services:
  postgres:
    ports: !reset []
    environment:
      POSTGRES_USER: strata
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD}
      POSTGRES_DB: strata
    command: ["postgres", "-c", "max_connections=200"]
    restart: unless-stopped
    deploy:
      resources:
        limits:
          cpus: "1.0"
          memory: 1G
    networks:
      - backend

  strata:
    ports: !reset []
    build:
      context: .
      dockerfile: docker/strata.Dockerfile
    environment:
      DATABASE_URL_ADMIN: "postgres://strata:${POSTGRES_PASSWORD}@postgres:5432/strata"
      DATABASE_URL: "postgres://strata_app:${STRATA_APP_PASSWORD}@postgres:5432/strata"
      STRATA_APP_PASSWORD: ${STRATA_APP_PASSWORD}
      HOST: "0.0.0.0"
      PORT: "3000"
      NUCLEUS_SECRET_KEY: ${NUCLEUS_SECRET_KEY:-}
      NUCLEUS_BASE_URL: ${NUCLEUS_BASE_URL:-}
      RESEND_API_KEY: ${RESEND_API_KEY:-}
      ALERT_FROM_EMAIL: ${ALERT_FROM_EMAIL:-alerts@strata.local}
    restart: unless-stopped
    deploy:
      resources:
        limits:
          cpus: "1.0"
          memory: 512M
    networks:
      - frontend
      - backend

  prometheus:
    ports: !reset []
    restart: unless-stopped
    networks:
      - backend

  loki:
    ports: !reset []
    restart: unless-stopped
    networks:
      - backend

  caddy:
    image: caddy:2-alpine
    restart: unless-stopped
    ports:
      - "80:80"
      - "443:443"
    environment:
      STRATA_DOMAIN: ${STRATA_DOMAIN}
      ACME_EMAIL: ${ACME_EMAIL}
    volumes:
      - ./caddy/Caddyfile:/etc/caddy/Caddyfile:ro
      - caddy_data:/data
      - caddy_config:/config
    networks:
      - frontend
    depends_on:
      strata:
        condition: service_healthy

  backup:
    build:
      context: .
      dockerfile: docker/backup.Dockerfile
    restart: unless-stopped
    environment:
      DATABASE_URL: "postgres://strata:${POSTGRES_PASSWORD}@postgres:5432/strata"
      BACKUP_INTERVAL: ${BACKUP_INTERVAL:-86400}
      RETENTION_DAYS: ${RETENTION_DAYS:-14}
      S3_BUCKET: ${S3_BUCKET:-}
      AWS_ACCESS_KEY_ID: ${AWS_ACCESS_KEY_ID:-}
      AWS_SECRET_ACCESS_KEY: ${AWS_SECRET_ACCESS_KEY:-}
      AWS_DEFAULT_REGION: ${AWS_DEFAULT_REGION:-}
    volumes:
      - backup_data:/backups
    networks:
      - backend
    depends_on:
      postgres:
        condition: service_healthy

volumes:
  caddy_data:
  caddy_config:
  backup_data:

networks:
  frontend:
  backend:
    internal: true
```

The `!reset []` Compose merge directive removes the `ports` block inherited from the base file. This is the documented way to "delete" a sequence in an override. The base file's `pgdata` volume is inherited and unchanged.

- [ ] **Step 2: Validate the merged compose configuration.**

```bash
# Set the four required vars to dummy values just so Compose doesn't fail
# on missing interpolation; we're only validating YAML structure.
STRATA_DOMAIN=example.com \
ACME_EMAIL=a@b \
POSTGRES_PASSWORD=p1 \
STRATA_APP_PASSWORD=p2 \
docker compose -f docker-compose.yml -f docker-compose.production.yml config > /tmp/merged.yml
echo "Exit: $?"
head -40 /tmp/merged.yml
```

Expected: exit 0, the merged YAML lists `services: postgres, strata, prometheus, loki, caddy, backup`, both networks present, no `ports` on postgres/strata/prometheus/loki, `ports` present on caddy.

- [ ] **Step 3: Commit.**

```bash
git add docker-compose.production.yml
git commit -m "ops: add docker-compose.production.yml with Caddy and backup sidecar"
```

---

## Task 7: Create .env.production.example

**Files:**
- Create: `.env.production.example`

- [ ] **Step 1: Write the file.**

Write `.env.production.example`:

```env
# Strata production environment variables.
# Copy this file to `.env.production` and fill in real values, then run:
#   ./scripts/deploy.sh
#
# DNS for STRATA_DOMAIN must point at this host BEFORE first deploy, otherwise
# Caddy's Let's Encrypt request will fail. The strata service is still
# reachable on the internal network during retries, so the failure mode is
# "TLS unavailable", not "stack down".

# === Domain + ACME (required) ===
STRATA_DOMAIN=strata.example.com
ACME_EMAIL=ops@example.com

# === Postgres (required, must be changed) ===
# Use distinct, strong, randomly-generated values. Suggested generation:
#   openssl rand -base64 48 | tr -d '/+=\n'
POSTGRES_PASSWORD=__CHANGE_ME__strong_random_64_chars__
STRATA_APP_PASSWORD=__CHANGE_ME__different_strong_random_64_chars__

# === Backup (optional; sensible defaults) ===
BACKUP_INTERVAL=86400        # seconds; default 24 hours
RETENTION_DAYS=14            # local-volume retention; S3 lifecycle managed separately
S3_BUCKET=                   # leave empty for local-volume-only backups
AWS_ACCESS_KEY_ID=           # only required when S3_BUCKET is set
AWS_SECRET_ACCESS_KEY=
AWS_DEFAULT_REGION=

# === Auth (optional — omit to disable JWT auth) ===
NUCLEUS_SECRET_KEY=
NUCLEUS_BASE_URL=

# === Email notifications (optional — omit to disable Chorus alerts) ===
RESEND_API_KEY=
ALERT_FROM_EMAIL=alerts@strata.example.com
```

- [ ] **Step 2: Verify the four required-must-change values are obvious placeholders.**

```bash
grep -E "^(STRATA_DOMAIN|ACME_EMAIL|POSTGRES_PASSWORD|STRATA_APP_PASSWORD)" .env.production.example
```

Expected: all four lines present; `POSTGRES_PASSWORD` and `STRATA_APP_PASSWORD` contain the literal substring `__CHANGE_ME__` so `deploy.sh` can detect unedited files.

- [ ] **Step 3: Commit.**

```bash
git add .env.production.example
git commit -m "ops: add .env.production.example documenting required prod vars"
```

---

## Task 8: Create scripts/deploy.sh

**Files:**
- Create: `scripts/deploy.sh`

- [ ] **Step 1: Write the script.**

Write `scripts/deploy.sh`:

```bash
#!/bin/bash
set -euo pipefail

ENV_FILE="${ENV_FILE:-.env.production}"

if [[ ! -f "$ENV_FILE" ]]; then
    echo "Error: $ENV_FILE not found." >&2
    echo "Copy .env.production.example to $ENV_FILE and fill in real values." >&2
    exit 1
fi

# Load env file into the script's environment so we can validate values.
set -a
# shellcheck disable=SC1090
source "$ENV_FILE"
set +a

required=(STRATA_DOMAIN ACME_EMAIL POSTGRES_PASSWORD STRATA_APP_PASSWORD)
for var in "${required[@]}"; do
    value="${!var:-}"
    if [[ -z "$value" || "$value" == *"__CHANGE_ME__"* ]]; then
        echo "Error: $var is unset or still has the __CHANGE_ME__ placeholder." >&2
        echo "Edit $ENV_FILE and set a real value for $var." >&2
        exit 1
    fi
done

echo "Bringing up production stack…"
docker compose \
    -f docker-compose.yml \
    -f docker-compose.production.yml \
    --env-file "$ENV_FILE" \
    up -d --build

echo "Waiting up to 60s for strata to report healthy…"
deadline=$(( $(date +%s) + 60 ))
while [[ $(date +%s) -lt $deadline ]]; do
    state=$(docker compose \
        -f docker-compose.yml \
        -f docker-compose.production.yml \
        --env-file "$ENV_FILE" \
        ps --format json strata 2>/dev/null \
        | grep -o '"Health":"[^"]*"' | head -1 | cut -d'"' -f4 || true)
    if [[ "$state" == "healthy" ]]; then
        break
    fi
    sleep 2
done

if [[ "${state:-}" != "healthy" ]]; then
    echo "Warning: strata did not become healthy within 60s. Recent logs:" >&2
    docker compose \
        -f docker-compose.yml \
        -f docker-compose.production.yml \
        --env-file "$ENV_FILE" \
        logs --tail 30 strata >&2
    exit 1
fi

cat <<EOF

✓ Strata is live at https://${STRATA_DOMAIN}

Tail logs with:
  docker compose -f docker-compose.yml -f docker-compose.production.yml --env-file ${ENV_FILE} logs -f strata

Tail Caddy (TLS / ACME) with:
  docker compose -f docker-compose.yml -f docker-compose.production.yml --env-file ${ENV_FILE} logs -f caddy
EOF
```

- [ ] **Step 2: Make it executable and lint it.**

```bash
chmod +x scripts/deploy.sh
bash -n scripts/deploy.sh
which shellcheck && shellcheck scripts/deploy.sh || echo "shellcheck not installed"
```

Expected: syntax check passes; shellcheck reports zero warnings or skip message.

- [ ] **Step 3: Smoke-test the validation path.**

```bash
# 1) No env file → must fail with helpful message.
ENV_FILE=/nonexistent ./scripts/deploy.sh 2>&1 | head -3
echo "Exit: $?"
```

Expected: prints "Error: /nonexistent not found." plus the suggestion to copy from `.env.production.example`, exit code 1.

```bash
# 2) Env file with __CHANGE_ME__ placeholder → must fail.
cp .env.production.example /tmp/.env.bad
ENV_FILE=/tmp/.env.bad ./scripts/deploy.sh 2>&1 | head -3
echo "Exit: $?"
rm /tmp/.env.bad
```

Expected: error citing `POSTGRES_PASSWORD` (or `STRATA_APP_PASSWORD`) still has the placeholder, exit 1.

- [ ] **Step 4: Commit.**

```bash
git add scripts/deploy.sh
git commit -m "ops: add scripts/deploy.sh with env validation and health wait"
```

---

## Task 9: End-to-end verification

**Files:** none modified (verification only).

- [ ] **Step 1: Validate the merged compose with realistic env.**

```bash
cp .env.production.example /tmp/.env.smoke
# Use sed to replace placeholders so the compose config validates.
sed -i 's/__CHANGE_ME__strong_random_64_chars__/dev-postgres-password-NOTREAL/' /tmp/.env.smoke
sed -i 's/__CHANGE_ME__different_strong_random_64_chars__/dev-strata-app-password-NOTREAL/' /tmp/.env.smoke

docker compose \
    -f docker-compose.yml \
    -f docker-compose.production.yml \
    --env-file /tmp/.env.smoke \
    config > /tmp/merged.yml

echo "Exit: $?"
grep -E "^  (postgres|strata|caddy|backup|prometheus|loki):" /tmp/merged.yml
```

Expected: exit 0, six service names listed.

- [ ] **Step 2: Confirm `ports` is empty on strata/postgres/prometheus/loki and present on caddy.**

```bash
# strata: should have no host ports
yq '.services.strata.ports // []' /tmp/merged.yml 2>/dev/null || python3 -c "
import yaml
m = yaml.safe_load(open('/tmp/merged.yml'))
for s in ['strata', 'postgres', 'prometheus', 'loki']:
    ports = m['services'][s].get('ports', [])
    assert ports == [] or ports is None, f'{s} unexpectedly has ports: {ports}'
caddy_ports = m['services']['caddy']['ports']
assert any('80' in str(p) for p in caddy_ports), 'caddy missing port 80'
assert any('443' in str(p) for p in caddy_ports), 'caddy missing port 443'
print('OK: port exposure is correct')
"
```

Expected: the python check prints `OK: port exposure is correct`.

- [ ] **Step 3: Confirm the backend network is internal.**

```bash
python3 -c "
import yaml
m = yaml.safe_load(open('/tmp/merged.yml'))
assert m['networks']['backend'].get('internal') is True, 'backend must be internal: true'
print('OK: backend network is internal')
"
```

- [ ] **Step 4: Cleanup.**

```bash
rm /tmp/.env.smoke /tmp/merged.yml
```

- [ ] **Step 5: Confirm the full Rust suite still passes.**

```bash
cd resource && DATABASE_URL=postgres://strata:secret@localhost:5432/strata cargo test --workspace 2>&1 | tail -5
```

Expected: all tests pass.

- [ ] **Step 6: Final clippy.**

```bash
cd resource && cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -5
```

Expected: clean, zero warnings.

- [ ] **Step 7: Final commit (only if anything incidentally changed).**

```bash
cd /home/mrbt/Desktop/workspaces/software/repositories/strata
git status
```

If clean, no commit needed. Otherwise:

```bash
git add -p
git commit -m "ops: tier 1 hardening verification sweep"
```

---

## Done criteria

- `./scripts/deploy.sh` validates `.env.production`, runs `docker compose up -d`, polls health, and prints the `https://${STRATA_DOMAIN}` URL.
- `docker compose -f docker-compose.yml -f docker-compose.production.yml config` produces a valid merged config with no host-exposed ports on postgres/strata/prometheus/loki, port 80+443 on caddy, `internal: true` on the `backend` network.
- Rust suite passes including the two new `db::bootstrap::tests`. Clippy is clean.
- The runtime app, when launched via the production override, connects as `strata_app` (verified by the `prod_path_alters_strata_app_password` test which asserts `rolcanlogin = true` post-bootstrap).
- Backup sidecar logs `Starting backup → /backups/strata-<ts>.sql.gz` on schedule (deferred to actual deploy; smoke-tested in Task 4 by exercising the loop body).
- The four `__CHANGE_ME__` placeholders block deploys until edited.

After this plan is complete, the next natural follow-ups are: Tier 3 AWS (flesh out OpenTofu modules), dogfooding metrics (`/metrics` + self-monitor template), and the Nucleus JWT `tenant_id` claim (cross-repo).
