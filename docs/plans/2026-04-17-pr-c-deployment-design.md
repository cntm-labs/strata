# PR C: Deployment Architecture Design Doc — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Write a comprehensive deployment architecture design document covering 4 deployment tiers, multi-tenant decisions, and monitoring stack.

**Architecture:** Design doc only — no code implementation. The doc will guide future PRs for each tier.

**Tech Stack:** Markdown documentation. References: AWS (ECS, RDS, ALB, CloudWatch), Kubernetes/Helm, Docker Compose, Terraform/CDK.

---

### Task 1: Write the deployment architecture design doc

**Files:**
- Create: `docs/deployment-architecture.md`

**Step 1: Write the document**

```markdown
# Strata Deployment Architecture

> Design document — guides future implementation PRs for each tier.

## Deployment Tiers

### Tier 1: Docker Compose (Dev / Small Teams)

**Status:** Exists (`docker-compose.yml`). Needs production hardening.

**Current state:**
- PostgreSQL 16, Prometheus, Loki, Strata
- No TLS, no resource limits, no backup strategy

**Production hardening TODO:**
- Add `docker-compose.production.yml` override:
  - Resource limits (CPU/memory) for all services
  - `restart: unless-stopped` on all services
  - Named networks with internal isolation
  - PostgreSQL `max_connections` tuning
- Add Caddy reverse proxy for automatic TLS (Let's Encrypt)
- Add PostgreSQL backup cron via `pg_dump` to S3/local volume
- Add `.env.production` example with required secrets
- Add `scripts/deploy.sh` for single-command production start

**Target users:** Solo developers, small startups, on-premise labs.

---

### Tier 4: Multi-Tenant Architecture Decision

**Status:** Design decision required before Tier 3.

**Must decide:** How to isolate tenant data in PostgreSQL.

#### Option A: Shared Database + Row-Level Security (Recommended)

```
┌─────────────────────────────────┐
│         Single RDS Instance      │
│  ┌───────────────────────────┐  │
│  │   dashboards              │  │
│  │   + tenant_id (UUID, FK)  │  │
│  │   + RLS policy per table  │  │
│  └───────────────────────────┘  │
│  ┌───────────────────────────┐  │
│  │   tenants                 │  │
│  │   id, name, slug, plan    │  │
│  └───────────────────────────┘  │
└─────────────────────────────────┘
```

**Pros:**
- Single RDS instance — lowest cost
- Simple migrations (one schema)
- PostgreSQL RLS is battle-tested
- Easy cross-tenant analytics (admin)

**Cons:**
- Noisy neighbor risk (mitigate with connection pooling)
- All tenants share IOPS budget
- Data breach blast radius = all tenants

**Implementation:**
- Add `tenant_id UUID` column to all tables
- Add `tenants` table
- Enable RLS: `CREATE POLICY tenant_isolation ON dashboards USING (tenant_id = current_setting('app.tenant_id')::UUID)`
- Set `app.tenant_id` in middleware from JWT claims

#### Option B: Schema-Per-Tenant

```
┌─────────────────────────────────┐
│         Single RDS Instance      │
│  ┌──────────┐ ┌──────────┐     │
│  │ tenant_a │ │ tenant_b │ ... │
│  │ .dashb.  │ │ .dashb.  │     │
│  │ .panels  │ │ .panels  │     │
│  └──────────┘ └──────────┘     │
└─────────────────────────────────┘
```

**Pros:**
- Stronger isolation than RLS
- Easy per-tenant backup/restore
- No RLS complexity

**Cons:**
- Migration complexity (N schemas to migrate)
- Connection pooling complexity (schema switching)
- Harder cross-tenant queries

#### Option C: Database-Per-Tenant

**Pros:** Full isolation, easy compliance.
**Cons:** High cost (N RDS instances), complex routing, slow tenant provisioning.

**Not recommended** unless regulatory requirements demand it.

#### Recommendation

**Option A (Shared DB + RLS)** for initial SaaS launch:
- Lowest cost and complexity
- Scale to ~1000 tenants on single RDS
- Migrate to Option B if noisy-neighbor becomes issue
- Migrate to Option C only for enterprise compliance needs

---

### Tier 3: AWS Managed (ECS Fargate)

**Status:** Not implemented. Design below.

**Depends on:** Tier 4 decision (affects RDS configuration).

```
┌──────────────────────────────────────────────┐
│                    AWS VPC                     │
│                                                │
│  ┌──────────┐    ┌─────────────────────────┐  │
│  │   ALB    │───▶│  ECS Fargate Service    │  │
│  │  + ACM   │    │  ┌───────┐ ┌───────┐   │  │
│  │  (TLS)   │    │  │Strata │ │Strata │   │  │
│  └──────────┘    │  │ task  │ │ task  │   │  │
│                   │  └───┬───┘ └───┬───┘   │  │
│                   └──────┼─────────┼───────┘  │
│                          │         │          │
│                   ┌──────▼─────────▼───────┐  │
│                   │    RDS PostgreSQL       │  │
│                   │    (Multi-AZ)           │  │
│                   └────────────────────────┘  │
│                                                │
│  ┌──────────────┐  ┌──────────────────────┐  │
│  │ ECR / GHCR   │  │ CloudWatch Logs      │  │
│  │ (images)     │  │ + Metrics            │  │
│  └──────────────┘  └──────────────────────┘  │
└──────────────────────────────────────────────┘
```

**Components:**
- **ECS Fargate:** Strata containers (min 2 tasks for HA)
- **ALB:** Load balancer with ACM TLS certificate
- **RDS PostgreSQL:** Multi-AZ, automated backups, encrypted
- **ECR:** Container registry (or continue using GHCR)
- **CloudWatch:** Logs from ECS tasks + RDS metrics
- **Secrets Manager:** DATABASE_URL, NUCLEUS_SECRET_KEY, RESEND_API_KEY
- **Route 53:** DNS management

**Infrastructure as Code:** Terraform modules:
- `modules/networking` — VPC, subnets, security groups
- `modules/database` — RDS instance, parameter group
- `modules/service` — ECS cluster, service, task definition
- `modules/loadbalancer` — ALB, target group, ACM cert
- `modules/monitoring` — CloudWatch dashboards, alarms

**Auto-scaling:**
- ECS Service Auto Scaling based on CPU (target 70%)
- RDS read replicas for read-heavy workloads (future)

**CI/CD integration:**
- GitHub Actions `release.yml` already pushes to GHCR
- Add: ECS deploy step after image push (rolling update)

---

### Tier 2: Kubernetes / Helm (Enterprise Self-Hosted)

**Status:** Not implemented. Lower priority than Tier 3.

**Helm chart structure:**
```
charts/strata/
├── Chart.yaml
├── values.yaml
├── templates/
│   ├── deployment.yaml
│   ├── service.yaml
│   ├── ingress.yaml
│   ├── configmap.yaml
│   ├── secret.yaml
│   ├── hpa.yaml
│   └── pdb.yaml
└── charts/
    └── postgresql/     # Bitnami subchart
```

**Key values.yaml settings:**
- `image.repository` / `image.tag`
- `postgresql.enabled` (true = subchart, false = external)
- `postgresql.auth.database` / `existingSecret`
- `ingress.enabled` / `ingress.className` / `ingress.hosts`
- `autoscaling.enabled` / `autoscaling.minReplicas` / `autoscaling.maxReplicas`
- `nucleus.secretKey` / `resend.apiKey` (via Secret)

**Target users:** Enterprise teams with existing k8s clusters.

---

## Monitoring Stack (Cross-Cutting)

### Dogfooding: Strata Monitors Strata

**New Prometheus metrics to expose from Strata backend:**

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `strata_http_requests_total` | counter | method, path, status | HTTP request count |
| `strata_http_request_duration_seconds` | histogram | method, path | Request latency |
| `strata_active_connections` | gauge | — | Current DB pool connections |
| `strata_query_proxy_total` | counter | datasource_type, status | Proxied query count |
| `strata_query_proxy_duration_seconds` | histogram | datasource_type | Proxy query latency |
| `strata_alerts_fired_total` | counter | severity | Alert fire count |
| `strata_email_sent_total` | counter | status | Chorus email notifications |

**Implementation:**
- Add `metrics-exporter-prometheus` crate
- Expose `GET /metrics` endpoint (public, no auth)
- Add `strata-health` dashboard template (self-monitor)
- Prometheus scrape config in docker-compose

### External: Production Alerting

| Tool | Purpose | When to Add | Integration |
|------|---------|-------------|-------------|
| **CloudWatch** | AWS infra metrics | Tier 3 | Built-in with ECS/RDS |
| **Datadog** | APM, distributed tracing | Tier 3+ | `dd-trace` sidecar in ECS |
| **PagerDuty** | On-call alert routing | All tiers | Strata alert → PagerDuty API |
| **Sentry** | Error/panic capture | All tiers | `sentry-rust` crate |

**Priority order:**
1. Sentry (error tracking) — add first, works all tiers
2. CloudWatch (comes free with Tier 3)
3. PagerDuty (alert routing)
4. Datadog (APM — most complex, add last)

**Goal:** Use these tools to understand what Strata needs to build natively. When Strata can replace a tool's function, remove the external dependency.

---

## Implementation Roadmap

```
Now     ──▶  PR C (this doc)
             │
Next    ──▶  Tier 1 hardening PR
             │
Then    ──▶  Tier 4 decision PR (add tenant_id, RLS)
             │
Then    ──▶  Tier 3 PR (Terraform + ECS deploy)
             │
Then    ──▶  Dogfooding PR (Prometheus metrics + self-monitor template)
             │
Then    ──▶  Sentry integration PR
             │
Future  ──▶  Tier 2 PR (Helm chart)
             │
Future  ──▶  PagerDuty + Datadog integration
```

Each step is a separate PR with its own plan.
```

**Step 2: Commit**

```bash
git add docs/deployment-architecture.md
git commit -m "docs: add deployment architecture design (4 tiers + monitoring)"
```

---

### Task 2: Update CLAUDE.md with deployment references

**Files:**
- Modify: `CLAUDE.md`

**Step 1: Add deployment section**

After the "Related Projects" section at the end, add:

```markdown
## Deployment
- **Deployment architecture:** See `docs/deployment-architecture.md`
- **Docker Compose:** `docker-compose.yml` (dev), `docker-compose.production.yml` (production, TODO)
- **Container registry:** `ghcr.io/cntm-labs/strata` (pushed on release tags)
- **Kubernetes:** Helm chart planned (see deployment architecture doc)
- **SaaS:** Multi-tenant architecture designed, not yet implemented
```

**Step 2: Run tests to verify no breakage**

Run: `cd resource && DATABASE_URL=postgres://strata:secret@localhost:5432/strata cargo test`
Expected: all pass

**Step 3: Commit**

```bash
git add CLAUDE.md docs/deployment-architecture.md
git commit -m "docs: add deployment architecture design and update CLAUDE.md"
```

---

### Task 3: Close related issues with comments

**Step 1: Comment on issues noting deployment design**

No GitHub issues to close for PR C — this is proactive design work.

**Step 2: Final verification**

Run: `cargo clippy -- -D warnings && cargo fmt --all --check`
Expected: clean (no Rust changes in this PR)
