-- Create tenants table
CREATE TABLE tenants (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    slug VARCHAR(100) NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Add tenant_id to dashboards, panels, and alert tables
ALTER TABLE dashboards ADD COLUMN tenant_id UUID REFERENCES tenants(id);
ALTER TABLE panels ADD COLUMN tenant_id UUID REFERENCES tenants(id);
ALTER TABLE alert_rules ADD COLUMN tenant_id UUID REFERENCES tenants(id);
ALTER TABLE alert_events ADD COLUMN tenant_id UUID REFERENCES tenants(id);

-- Enable Row Level Security
ALTER TABLE dashboards ENABLE ROW LEVEL SECURITY;
ALTER TABLE panels ENABLE ROW LEVEL SECURITY;
ALTER TABLE alert_rules ENABLE ROW LEVEL SECURITY;
ALTER TABLE alert_events ENABLE ROW LEVEL SECURITY;

-- Create RLS policies
CREATE POLICY tenant_policy ON dashboards 
    FOR ALL 
    USING (tenant_id = current_setting('app.tenant_id')::UUID);

CREATE POLICY tenant_policy ON panels 
    FOR ALL 
    USING (tenant_id = current_setting('app.tenant_id')::UUID);

CREATE POLICY tenant_policy ON alert_rules 
    FOR ALL 
    USING (tenant_id = current_setting('app.tenant_id')::UUID);

CREATE POLICY tenant_policy ON alert_events 
    FOR ALL 
    USING (tenant_id = current_setting('app.tenant_id')::UUID);
