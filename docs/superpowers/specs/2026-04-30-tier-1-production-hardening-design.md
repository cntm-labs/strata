# Tier 1 Production Hardening — Design

**Date:** 2026-04-30
**Status:** Draft → ready for implementation plan
**Scope:** Repository root (compose files, scripts, env templates) + small backend change in `resource/core/main.rs`
**Out of scope:** ORM migration to `cntm-labs/sentinel` (separate future PR), multi-host orchestration (Tier 2), restore tooling, replication, public exposure of Prometheus/Loki

---

## Problem Statement

PR #13 added Row-Level Security with `FORCE` and a `strata_app` non-super role. The role exists but it has `NOLOGIN` and no password, so the runtime app cannot connect as it. The runtime today still uses the migration-time superuser (`strata`), which bypasses RLS regardless of `FORCE` — meaning tenant isolation is enforced in tests but **silently disabled in any compose-deployed environment**.

The current `docker-compose.yml` is also dev-only: it exposes Postgres on the host, hardcodes a weak password, has no resource limits, no restart policy, no TLS, no backup, and no production override. There is no `scripts/deploy.sh` and no `.env.production.example`.

This work closes both gaps in one change so a Tier 1 deployment ("single-host, small team / on-prem lab") is operable end-to-end and the RLS guarantees from PR #13 actually take effect.

## Goals

- The runtime app connects as `strata_app`. RLS is enforced at the database layer in production exactly as it is in tests.
- Production deploy is one command: `./scripts/deploy.sh`. The script validates the env file, brings the stack up, and surfaces the domain + log tail.
- Caddy terminates TLS at the edge with automatic Let's Encrypt certificates. The Strata HTTP port is no longer exposed to the host.
- Postgres is no longer exposed to the host. The credentials in `.env.production` are not the dev defaults.
- Postgres is backed up daily by a sidecar container. Backups land on a local Docker volume by default; S3 upload is opt-in via `S3_BUCKET`.
- Adding a Tier 2 (Helm) or Tier 3 (AWS) deployment in a later PR does not require revisiting the password-bootstrap or RLS path. The same two-pool pattern transplants.

## Non-Goals

- Replacing sqlx with `cntm-labs/sentinel`. Sentinel's migration crate (`sntl-migrate`) is marked planned in its README; the swap is its own PR scoped after sntl-migrate ships.
- Public exposure of Prometheus or Loki. Operators reach them via SSH tunnel or VPN.
- Multi-host orchestration, HA Postgres, or read replicas — those belong to Tier 2/3 designs.
- Automated backup restore tooling. The first cut documents the manual `gunzip | psql` procedure; a `restore.sh` is a follow-up.
- Renewal monitoring or alerting on certificate expiry. Caddy renews automatically; observability for renewal is a future Sentry/Prom dashboard concern.

## Design

### 1. Compose layout — base + override

The existing `docker-compose.yml` stays as the dev definition (plain HTTP, exposed Postgres, hardcoded `secret`, no Caddy, no backup). A new `docker-compose.production.yml` overrides selected services and adds two new ones.

Production deploy: `docker compose -f docker-compose.yml -f docker-compose.production.yml --env-file .env.production up -d --build`. The `scripts/deploy.sh` wrapper inserts the file flags and the env file path so operators only type one command.

The override changes are:

- **`postgres`**: drop the `ports` block (no host exposure). Override `environment` to read `POSTGRES_PASSWORD` from env. Append `command: ["postgres", "-c", "max_connections=200"]`. Add `deploy.resources.limits` (1 CPU, 1 GiB) and `restart: unless-stopped`. Move it to the `backend` network only.
- **`strata`**: drop the `ports` block. Construct `DATABASE_URL_ADMIN` and `DATABASE_URL` in the override's `environment:` block via Compose YAML interpolation — e.g. `DATABASE_URL: "postgres://strata_app:${STRATA_APP_PASSWORD}@postgres:5432/strata"`. Pass `STRATA_APP_PASSWORD` through directly so the app can `ALTER ROLE` with it. Add `restart: unless-stopped` and CPU/memory limits (1 CPU, 512 MiB). Attach to both `frontend` (for Caddy) and `backend` (for Postgres).
- **`prometheus`, `loki`**: drop the `ports` blocks. Move to `backend` only. Add `restart: unless-stopped`. Operators reach them via tunnel.
- **`caddy`** (new): `image: caddy:2-alpine`, mounts `./caddy/Caddyfile:/etc/caddy/Caddyfile:ro`, persists ACME state in two named volumes `caddy_data` and `caddy_config`. Maps host `80:80` and `443:443`. Reads `STRATA_DOMAIN` and `ACME_EMAIL` from env (Caddyfile uses Caddy's env-substitution syntax). Attached to `frontend` only.
- **`backup`** (new): builds `docker/backup.Dockerfile`. Runs the rewritten `scripts/backup.sh` as a cron loop. Mounts a `backup_data` volume at `/backups`. Receives `DATABASE_URL_ADMIN` (read-only `pg_dump` needs login as a role with `pg_read_all_data` or the owner role; the migration role works), `BACKUP_INTERVAL`, `RETENTION_DAYS`, and optional `S3_BUCKET` + AWS credentials. Attached to `backend` only.

Three networks are defined: `frontend` (default-bridge, Caddy ↔ Strata), `backend` (`internal: true`, Strata ↔ Postgres ↔ backup, Prometheus, Loki), and the default network is unused. `internal: true` on `backend` blocks egress for services attached only to it; this prevents an exploited Postgres or backup container from reaching the public network.

### 2. Two-pool app bootstrap

`resource/core/main.rs` gains a deterministic startup sequence that establishes the role separation. The shape:

```rust
let admin_url = config.database_url_admin.as_deref()
    .unwrap_or(&config.database_url);
let admin_pool = PgPool::connect(admin_url).await?;

sqlx::migrate!("./migrations").run(&admin_pool).await?;

if let Some(password) = config.strata_app_password.as_deref() {
    sqlx::query("ALTER ROLE strata_app WITH LOGIN PASSWORD $1")
        .bind(password)
        .execute(&admin_pool)
        .await?;
}

admin_pool.close().await;

let runtime_pool = PgPoolOptions::new()
    .max_connections(20)
    .connect(&config.database_url)
    .await?;
```

`AppConfig` gains two optional fields: `database_url_admin: Option<String>` and `strata_app_password: Option<String>`. When both are unset (dev), `admin_url` falls back to `DATABASE_URL` and the `ALTER ROLE` step is skipped — the existing dev compose continues to work unchanged.

The `ALTER ROLE` is parameterized through sqlx bind, not string-formatted, so the password value is opaque to the SQL parser and cannot inject. The statement is idempotent: running it on every startup keeps the role's password in sync with the env var, so password rotation is "edit `.env.production`, restart the strata service".

The admin pool is closed before the runtime pool is created. `AppState::pool` only ever holds the strata_app connection — no handler can accidentally execute on the admin pool.

### 3. Caddyfile

A single-domain reverse proxy with automatic HTTPS:

```
{$STRATA_DOMAIN} {
    encode zstd gzip
    reverse_proxy strata:3000

    log {
        output stdout
        format json
    }
}

{
    email {$ACME_EMAIL}
}
```

Caddy substitutes `{$VAR}` from its process environment at startup, so the domain and ACME contact email come from `.env.production` via `docker compose --env-file`. The global `email` directive opts the deploy into Let's Encrypt certificate notifications. No additional config (no rate limits, no IP allowlists) — those are enabled per-tenant in a later admin-tooling PR.

### 4. Backup sidecar

`scripts/backup.sh` is rewritten from the current one-shot form to a cron loop:

```bash
#!/bin/bash
set -euo pipefail

: "${DATABASE_URL:?DATABASE_URL must be set}"
: "${BACKUP_INTERVAL:=86400}"
: "${RETENTION_DAYS:=14}"

while true; do
    ts=$(date -u +%Y%m%dT%H%M%SZ)
    file="/backups/strata-${ts}.sql.gz"
    echo "[$(date -u -Iseconds)] Starting backup → $file"
    pg_dump "$DATABASE_URL" | gzip > "$file"
    echo "[$(date -u -Iseconds)] Backup complete: $(du -h "$file" | cut -f1)"

    if [[ -n "${S3_BUCKET:-}" ]]; then
        aws s3 cp "$file" "s3://${S3_BUCKET}/backups/$(basename "$file")"
        echo "[$(date -u -Iseconds)] Uploaded to s3://${S3_BUCKET}/"
    fi

    find /backups -name 'strata-*.sql.gz' -mtime +"${RETENTION_DAYS}" -delete

    sleep "${BACKUP_INTERVAL}"
done
```

The sidecar logs each backup to stdout, which Docker captures via `docker logs strata-backup-1`. S3 upload is a no-op when `S3_BUCKET` is unset — operators on-prem don't need an AWS account.

The backup container connects as the migration role (a superuser), which has unconditional read access. Attempting to use `strata_app` for backups would either return zero rows for every tenant-scoped table (RLS filter) or, with proper `pg_read_all_data` grants, work but add complexity that doesn't pay back at Tier 1 scale.

### 5. `.env.production.example`

The new file documents every variable the production stack needs, with placeholder values that will obviously fail if left as-is:

```env
# Domain + ACME
STRATA_DOMAIN=strata.example.com
ACME_EMAIL=ops@example.com

# Postgres (strong, distinct passwords for the two roles)
# The two DATABASE_URL values are constructed in docker-compose.production.yml
# from these passwords — Compose does ${VAR} substitution in YAML at startup,
# so operators only manage the password values here.
POSTGRES_PASSWORD=__CHANGE_ME__strong_random_64_chars__
STRATA_APP_PASSWORD=__CHANGE_ME__different_strong_random_64_chars__

# Backup
BACKUP_INTERVAL=86400        # seconds; default 24 hours
RETENTION_DAYS=14
S3_BUCKET=                    # leave empty for local-volume-only backups
AWS_ACCESS_KEY_ID=            # only required if S3_BUCKET is set
AWS_SECRET_ACCESS_KEY=
AWS_DEFAULT_REGION=

# Auth + email (existing, unchanged)
NUCLEUS_SECRET_KEY=
NUCLEUS_BASE_URL=
RESEND_API_KEY=
ALERT_FROM_EMAIL=alerts@strata.example.com
```

### 6. `scripts/deploy.sh`

A small bash wrapper that:

1. Verifies `.env.production` exists.
2. Loads it and asserts the four "must change" variables (`STRATA_DOMAIN`, `ACME_EMAIL`, `POSTGRES_PASSWORD`, `STRATA_APP_PASSWORD`) are set and not equal to their `__CHANGE_ME__` placeholders.
3. Runs `docker compose -f docker-compose.yml -f docker-compose.production.yml --env-file .env.production up -d --build`.
4. Polls `docker compose ... ps --format json` until the strata service reports `running (healthy)` or 60 s elapse.
5. Prints `Strata is live at https://${STRATA_DOMAIN}` and `Tail logs with: docker compose -f docker-compose.yml -f docker-compose.production.yml --env-file .env.production logs -f strata`.

The script is idempotent — re-running it after editing `.env.production` is the supported way to rotate passwords or change the domain.

## Risks and Trade-offs

- **`ALTER ROLE` runs every startup.** Negligible cost (one statement against a freshly-pooled admin connection), and the idempotent shape means env-var rotation is the only password-change procedure operators need. The cost of the alternative — a separate one-shot init container — is more moving parts and an out-of-band rotation flow.
- **Admin pool open during startup.** A misconfigured app could hold the admin connection longer than necessary; the closing `admin_pool.close().await` is one line and is run unconditionally. A panic between `migrate.run()` and `close()` leaks the connection until process exit, which is acceptable for a startup-time pool that the OS reclaims anyway.
- **Backups use the superuser role.** This is the simplest correct choice for Tier 1. A future PR can switch the backup role to a dedicated `strata_backup` user with `pg_read_all_data`; flagged as a `TODO(prod)` in the new `backup.sh` so it shows up in `git grep`.
- **Caddy needs DNS pointing at the host before TLS works.** The first `up -d` will fail to obtain a cert if DNS is wrong. Caddy retries in the background and the strata service is still reachable over plain HTTP on the internal network during the retry window, so the failure mode is "TLS unavailable" not "stack down". Documented in `.env.production.example` as a comment.
- **Internal network with `internal: true` blocks the backup container's egress.** The backup container needs egress to reach S3 when `S3_BUCKET` is set. To preserve the isolation guarantee, the backup container is **also** attached to the `frontend` network when `S3_BUCKET` is set — but at Tier 1 we accept the simpler shape: backup is on `backend` only, and S3 upload requires the operator to add a third egress-capable network in their `.env.production` if they want it. The default (local-volume backups) needs no egress.

  Implementation note: making this seamless would require a fourth network layer or a per-environment compose file. We pick the simpler path and document the limitation.

## Open Questions

None blocking. Items intentionally deferred:

- Whether to ship a `restore.sh` companion. The procedure is `gunzip < strata-<ts>.sql.gz | psql "$DATABASE_URL_ADMIN"`; documenting it inline in the main README's deployment section is enough for the first cut.
- Whether to expose Prometheus and Loki via a separate authenticated subdomain (e.g. `metrics.strata.example.com`). Future PR if dogfooding metrics demands it.
- Whether to pre-bake a `caddy:2-alpine` image with our `Caddyfile` so first-boot doesn't depend on a bind mount. Not worth the registry churn at Tier 1.
