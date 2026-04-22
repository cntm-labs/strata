CREATE TABLE tenants (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Add tenant_id to existing tables
ALTER TABLE dashboards ADD COLUMN tenant_id UUID REFERENCES tenants(id);
ALTER TABLE panels ADD COLUMN tenant_id UUID REFERENCES tenants(id);
ALTER TABLE alerts ADD COLUMN tenant_id UUID REFERENCES tenants(id);

-- Enable RLS
ALTER TABLE dashboards ENABLE ROW LEVEL SECURITY;
ALTER TABLE panels ENABLE ROW LEVEL SECURITY;
ALTER TABLE alerts ENABLE ROW LEVEL SECURITY;

-- Create Policies
CREATE POLICY tenant_isolation_dashboards ON dashboards USING (tenant_id = current_setting('app.tenant_id')::UUID);
CREATE POLICY tenant_isolation_panels ON panels USING (tenant_id = current_setting('app.tenant_id')::UUID);
CREATE POLICY tenant_isolation_alerts ON alerts USING (tenant_id = current_setting('app.tenant_id')::UUID);
