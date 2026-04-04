import { api } from './client'
import type { Dashboard } from '@/types'

export interface DashboardTemplate {
  id: string
  slug: string
  name: string
  description?: string
  category: string
  thumbnail_url?: string
  dashboard_json: unknown
  required_datasource_type?: string
  is_active: boolean
  created_at: string
}

export const templatesApi = {
  list: () => api.get<DashboardTemplate[]>('/templates'),
  use: (slug: string, data: { title: string; slug: string; datasource_id?: string }) =>
    api.post<Dashboard>(`/templates/${slug}/use`, data),
}
