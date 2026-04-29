-- 005_multi_tenant_rls.sql
-- Adds the tenant registry, the tenant_id column and RLS policy to every
-- tenant-owned table, and the composite indexes the new query plans need.
-- Idempotent: safe to re-run after a partial earlier attempt.

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
-- TODO(prod): on populated tables this holds ACCESS EXCLUSIVE during FK
--             validation. For the first production deploy, replace with the
--             online variant: ADD COLUMN NULL; backfill in batches;
--             ADD CONSTRAINT ... NOT VALID; VALIDATE CONSTRAINT;
--             ADD CHECK (tenant_id IS NOT NULL) NOT VALID; VALIDATE; SET NOT NULL.
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
        EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', t);
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

-- 6. Application role for runtime queries.
-- The migration role is typically a SUPERUSER, and superusers bypass RLS even
-- with FORCE — so the runtime app must connect as a non-super, non-bypass role.
-- TODO(prod): point the app's DATABASE_URL at strata_app once a password is
--             set for it; until then production still runs as the migration
--             role and tenant isolation is NOT enforced at the DB layer.
-- Race-safe: when sqlx::test runs migrations in parallel against multiple
-- fresh databases, the SELECT-then-CREATE pattern is non-atomic and two
-- workers can both decide to CREATE, with the second one losing on the
-- pg_authid_rolname_index uniqueness constraint. Catch duplicate_object
-- (SQLSTATE 42710) and swallow it.
DO $$
BEGIN
    CREATE ROLE strata_app NOLOGIN NOSUPERUSER NOBYPASSRLS;
EXCEPTION WHEN duplicate_object THEN
    NULL;
END $$;
GRANT USAGE ON SCHEMA public TO strata_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO strata_app;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO strata_app;
ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO strata_app;
ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT USAGE, SELECT ON SEQUENCES TO strata_app;
