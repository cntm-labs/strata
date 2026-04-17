# PR B: Chorus Loki + Cost Templates — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add two new dashboard templates: `chorus-logs` (Loki) for log search/correlation and `chorus-costs` (PostgreSQL) for cost analytics.

**Architecture:** Single migration file inserts 2 new templates into `dashboard_templates`. No Rust code changes — only SQL and tests.

**Tech Stack:** PostgreSQL migration (SQL), LogQL queries, SQL queries, existing Strata template system.

**Closes:** #8, #9

---

### Task 1: Create migration for Loki + Cost templates

**Files:**
- Create: `resource/migrations/004_chorus_logs_costs_templates.sql`

**Step 1: Write the migration**

```sql
INSERT INTO dashboard_templates (slug, name, description, category, dashboard_json, required_datasource_type) VALUES
('chorus-logs', 'Chorus Logs', 'Chorus structured log search, volume tracking, and error detection via Loki', 'cpaas',
 '{
    "panels": [
      {
        "title": "Error Rate /s",
        "type": "stat",
        "query": "sum(rate({app=\"chorus\"} |= \"ERROR\" [1m]))",
        "position": {"x": 0, "y": 0, "w": 4, "h": 3}
      },
      {
        "title": "Warn Rate /s",
        "type": "stat",
        "query": "sum(rate({app=\"chorus\"} |= \"WARN\" [1m]))",
        "position": {"x": 4, "y": 0, "w": 4, "h": 3}
      },
      {
        "title": "Total Log Volume /s",
        "type": "stat",
        "query": "sum(rate({app=\"chorus\"} [1m]))",
        "position": {"x": 8, "y": 0, "w": 4, "h": 3}
      },
      {
        "title": "Log Volume by Level",
        "type": "timeseries",
        "query": "sum(rate({app=\"chorus\"} [1m])) by (level)",
        "position": {"x": 0, "y": 3, "w": 12, "h": 3}
      },
      {
        "title": "Chorus Logs",
        "type": "logs",
        "query": "{app=\"chorus\"}",
        "position": {"x": 0, "y": 6, "w": 12, "h": 6}
      }
    ]
  }',
 'loki'),
('chorus-costs', 'Chorus Cost Analytics', 'Message cost breakdown by channel, provider, and account from Chorus PostgreSQL database', 'cpaas',
 '{
    "panels": [
      {
        "title": "Total Spend (30d)",
        "type": "stat",
        "query": "SELECT SUM(cost_microdollars) / 1000000.0 AS cost FROM messages WHERE created_at >= NOW() - INTERVAL ''30 days''",
        "position": {"x": 0, "y": 0, "w": 4, "h": 3}
      },
      {
        "title": "SMS Cost (30d)",
        "type": "stat",
        "query": "SELECT SUM(cost_microdollars) / 1000000.0 AS cost FROM messages WHERE channel = ''sms'' AND created_at >= NOW() - INTERVAL ''30 days''",
        "position": {"x": 4, "y": 0, "w": 4, "h": 3}
      },
      {
        "title": "Email Cost (30d)",
        "type": "stat",
        "query": "SELECT SUM(cost_microdollars) / 1000000.0 AS cost FROM messages WHERE channel = ''email'' AND created_at >= NOW() - INTERVAL ''30 days''",
        "position": {"x": 8, "y": 0, "w": 4, "h": 3}
      },
      {
        "title": "Daily Cost Trend",
        "type": "timeseries",
        "query": "SELECT DATE(created_at) AS time, SUM(cost_microdollars) / 1000000.0 AS cost FROM messages WHERE created_at >= NOW() - INTERVAL ''30 days'' GROUP BY DATE(created_at) ORDER BY time",
        "position": {"x": 0, "y": 3, "w": 6, "h": 3}
      },
      {
        "title": "Cost by Provider",
        "type": "piechart",
        "query": "SELECT provider, SUM(cost_microdollars) / 1000000.0 AS cost FROM messages GROUP BY provider ORDER BY cost DESC",
        "position": {"x": 6, "y": 3, "w": 6, "h": 3}
      },
      {
        "title": "Top Spending Accounts",
        "type": "table",
        "query": "SELECT account_id, SUM(cost_microdollars) / 1000000.0 AS cost, COUNT(*) AS messages FROM messages GROUP BY account_id ORDER BY cost DESC LIMIT 20",
        "position": {"x": 0, "y": 6, "w": 12, "h": 4}
      }
    ]
  }',
 'postgresql');
```

Note: SQL strings inside JSONB use `''` (double single-quotes) for PostgreSQL string escaping inside the outer single-quoted string.

**Step 2: Verify migration runs**

Run: `cd resource && DATABASE_URL=postgres://strata:secret@localhost:5432/strata cargo test`
Expected: compilation succeeds, but template list test will fail (expects 6 templates, now 8)

---

### Task 2: Update template tests for new template count

**Files:**
- Modify: `resource/core/api/templates.rs` (tests section)

**Step 1: Update existing test count**

Change in `list_returns_seeded_templates`:
```rust
// Old:
assert_eq!(items.len(), 6);
// New:
assert_eq!(items.len(), 8);
```

Change in `list_excludes_inactive`:
```rust
// Old:
assert_eq!(items.len(), 6);
// New:
assert_eq!(items.len(), 8);
```

**Step 2: Add tests for new templates**

```rust
#[sqlx::test]
async fn chorus_logs_template_has_5_panels(pool: sqlx::PgPool) {
    let template = sqlx::query_as::<_, DashboardTemplate>(
        "SELECT * FROM dashboard_templates WHERE slug = 'chorus-logs'"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(template.required_datasource_type.as_deref(), Some("loki"));
    assert_eq!(template.category, "cpaas");

    let panels = template.dashboard_json
        .get("panels")
        .and_then(|p| p.as_array())
        .expect("chorus-logs should have panels array");
    assert_eq!(panels.len(), 5);

    // Verify logs panel type exists
    let has_logs = panels.iter()
        .any(|p| p.get("type").and_then(|t| t.as_str()) == Some("logs"));
    assert!(has_logs, "chorus-logs should have a logs panel");
}

#[sqlx::test]
async fn chorus_costs_template_has_6_panels(pool: sqlx::PgPool) {
    let template = sqlx::query_as::<_, DashboardTemplate>(
        "SELECT * FROM dashboard_templates WHERE slug = 'chorus-costs'"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(template.required_datasource_type.as_deref(), Some("postgresql"));
    assert_eq!(template.category, "cpaas");

    let panels = template.dashboard_json
        .get("panels")
        .and_then(|p| p.as_array())
        .expect("chorus-costs should have panels array");
    assert_eq!(panels.len(), 6);

    // Verify table panel type exists (top accounts)
    let has_table = panels.iter()
        .any(|p| p.get("type").and_then(|t| t.as_str()) == Some("table"));
    assert!(has_table, "chorus-costs should have a table panel");
}
```

**Step 3: Run tests**

Run: `cd resource && DATABASE_URL=postgres://strata:secret@localhost:5432/strata cargo test api::templates::tests -- --nocapture`
Expected: all pass

**Step 4: Commit**

```bash
git add resource/migrations/004_chorus_logs_costs_templates.sql resource/core/api/templates.rs
git commit -m "feat: add chorus-logs (Loki) and chorus-costs (PostgreSQL) templates

chorus-logs: 5 panels — error/warn rate, log volume, raw log stream
chorus-costs: 6 panels — spend stats, daily trend, provider/account breakdown

Closes #8, closes #9"
```

---

### Task 3: Update SITEMAP.md and run full verification

**Files:**
- Modify: `SITEMAP.md`

**Step 1: Update SITEMAP.md template count**

Update the total at the bottom:
```markdown
## Total: 18 frontend pages + 29 API endpoints + 8 dashboard templates
```

**Step 2: Run full backend tests**

Run: `cd resource && DATABASE_URL=postgres://strata:secret@localhost:5432/strata cargo test`
Expected: all pass

**Step 3: Run clippy + fmt**

Run: `cargo clippy -- -D warnings && cargo fmt --all --check`
Expected: clean

**Step 4: Commit**

```bash
git add SITEMAP.md
git commit -m "docs: update SITEMAP.md template count to 8"
```
