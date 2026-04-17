# PR A: Chorus Prometheus Template — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Update the existing `chorus-overview` dashboard template with 10 comprehensive panels covering all Chorus Prometheus metrics.

**Architecture:** Single migration file updates the `dashboard_json` of the existing `chorus-overview` template seeded by migration 002. No Rust code changes — only SQL and tests.

**Tech Stack:** PostgreSQL migration (SQL), PromQL queries, existing Strata template system.

**Closes:** #3, #4, #5, #6, #7

---

### Task 1: Create migration to update chorus-overview template

**Files:**
- Create: `resource/migrations/003_update_chorus_template.sql`

**Step 1: Write the migration**

```sql
UPDATE dashboard_templates
SET
  description = 'Chorus CPaaS: message throughput, provider health, queue depth, webhook delivery',
  dashboard_json = '{
    "panels": [
      {
        "title": "Total Messages",
        "type": "stat",
        "query": "sum(chorus_messages_total)",
        "position": {"x": 0, "y": 0, "w": 3, "h": 3}
      },
      {
        "title": "Delivery Success Rate",
        "type": "gauge",
        "query": "sum(rate(chorus_messages_total{status=\"delivered\"}[5m])) / sum(rate(chorus_messages_total[5m])) * 100",
        "position": {"x": 3, "y": 0, "w": 3, "h": 3}
      },
      {
        "title": "Queue Depth",
        "type": "stat",
        "query": "chorus_queue_depth",
        "position": {"x": 6, "y": 0, "w": 3, "h": 3}
      },
      {
        "title": "Webhook Success Rate",
        "type": "gauge",
        "query": "sum(rate(chorus_webhook_deliveries_total{status=\"success\"}[5m])) / sum(rate(chorus_webhook_deliveries_total[5m])) * 100",
        "position": {"x": 9, "y": 0, "w": 3, "h": 3}
      },
      {
        "title": "Message Rate",
        "type": "timeseries",
        "query": "sum(rate(chorus_messages_total[5m]))",
        "position": {"x": 0, "y": 3, "w": 6, "h": 3}
      },
      {
        "title": "Rate by Channel",
        "type": "timeseries",
        "query": "sum(rate(chorus_messages_total[5m])) by (channel)",
        "position": {"x": 6, "y": 3, "w": 6, "h": 3}
      },
      {
        "title": "Provider Error Rate",
        "type": "timeseries",
        "query": "sum(rate(chorus_provider_errors_total[5m])) by (channel)",
        "position": {"x": 0, "y": 6, "w": 6, "h": 3}
      },
      {
        "title": "Messages by Provider",
        "type": "piechart",
        "query": "sum(chorus_messages_total) by (provider)",
        "position": {"x": 6, "y": 6, "w": 6, "h": 3}
      },
      {
        "title": "Queue Depth Over Time",
        "type": "timeseries",
        "query": "chorus_queue_depth",
        "position": {"x": 0, "y": 9, "w": 6, "h": 3}
      },
      {
        "title": "Webhook Deliveries",
        "type": "timeseries",
        "query": "sum(rate(chorus_webhook_deliveries_total[5m])) by (event)",
        "position": {"x": 6, "y": 9, "w": 6, "h": 3}
      }
    ]
  }'
WHERE slug = 'chorus-overview';
```

**Step 2: Verify migration runs**

Run: `cd resource && DATABASE_URL=postgres://strata:secret@localhost:5432/strata cargo test`
Expected: all tests pass (migrations run automatically in sqlx::test)

---

### Task 2: Update template list test for new panel count

**Files:**
- Modify: `resource/core/api/templates.rs` (tests section)

**Step 1: Add test for updated chorus-overview**

Add in `#[cfg(test)] mod tests`:

```rust
#[sqlx::test]
async fn chorus_overview_has_10_panels(pool: sqlx::PgPool) {
    let template = sqlx::query_as::<_, DashboardTemplate>(
        "SELECT * FROM dashboard_templates WHERE slug = 'chorus-overview'"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let panels = template.dashboard_json
        .get("panels")
        .and_then(|p| p.as_array())
        .expect("chorus-overview should have panels array");
    assert_eq!(panels.len(), 10, "chorus-overview should have 10 panels");

    // Verify key panel types exist
    let types: Vec<&str> = panels.iter()
        .filter_map(|p| p.get("type").and_then(|t| t.as_str()))
        .collect();
    assert!(types.contains(&"stat"));
    assert!(types.contains(&"gauge"));
    assert!(types.contains(&"timeseries"));
    assert!(types.contains(&"piechart"));
}
```

**Step 2: Run tests**

Run: `cd resource && DATABASE_URL=postgres://strata:secret@localhost:5432/strata cargo test api::templates::tests -- --nocapture`
Expected: all pass including new test

**Step 3: Commit**

```bash
git add resource/migrations/003_update_chorus_template.sql resource/core/api/templates.rs
git commit -m "feat: update chorus-overview template with 10 panels

Covers all Chorus Prometheus metrics: message throughput, provider
health, queue depth, webhook delivery.

Closes #3, closes #4, closes #5, closes #6, closes #7"
```

---

### Task 3: Update SITEMAP.md and run full verification

**Files:**
- Modify: `SITEMAP.md` (if template count changed — it didn't, still 6 templates)

**Step 1: Run full backend tests**

Run: `cd resource && DATABASE_URL=postgres://strata:secret@localhost:5432/strata cargo test`
Expected: all pass

**Step 2: Run clippy + fmt**

Run: `cargo clippy -- -D warnings && cargo fmt --all --check`
Expected: clean

**Step 3: Commit any final fixes**

```bash
git add -A
git commit -m "chore: final verification for chorus template update"
```
