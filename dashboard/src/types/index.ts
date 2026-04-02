export interface Datasource {
  id: string;
  name: string;
  type: "prometheus" | "loki" | "postgresql";
  url: string;
  is_default: boolean;
  created_at: string;
}

export interface Dashboard {
  id: string;
  title: string;
  slug: string;
  description?: string;
  layout: PanelPosition[];
  time_range: string;
  refresh_interval: number;
  variables: TemplateVariable[];
  is_starred: boolean;
  created_at: string;
  updated_at: string;
}

export interface Panel {
  id: string;
  dashboard_id: string;
  title: string;
  type: PanelType;
  datasource_id?: string;
  query: string;
  config: Record<string, unknown>;
  position: PanelPosition;
  created_at: string;
  updated_at: string;
}

export type PanelType =
  | "timeseries"
  | "stat"
  | "gauge"
  | "table"
  | "bar"
  | "heatmap"
  | "logs"
  | "piechart";

export interface PanelPosition {
  x: number;
  y: number;
  w: number;
  h: number;
  i: string;
}

export interface TemplateVariable {
  name: string;
  label: string;
  query: string;
  datasource_id: string;
  type: "query" | "custom" | "interval";
  current: string;
  options: string[];
}

export interface AlertRule {
  id: string;
  name: string;
  datasource_id: string;
  query: string;
  condition: "gt" | "lt" | "eq" | "gte" | "lte";
  threshold: number;
  duration_secs: number;
  severity: "info" | "warning" | "critical";
  notification_channels: string[];
  notification_recipients: string[];
  is_active: boolean;
  current_state: "ok" | "firing" | "pending";
  created_at: string;
  updated_at: string;
}

export interface AlertEvent {
  id: string;
  rule_id: string;
  state: string;
  value?: number;
  message?: string;
  notified_via: string[];
  created_at: string;
}
