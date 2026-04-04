import { api } from './client'
import type { Dashboard, Panel } from '@/types'

export const dashboardsApi = {
  list: () => api.get<Dashboard[]>('/dashboards'),
  get: (slug: string) => api.get<Dashboard>(`/dashboards/${slug}`),
  create: (data: Partial<Dashboard>) => api.post<Dashboard>('/dashboards', data),
  update: (slug: string, data: Partial<Dashboard>) =>
    api.put<Dashboard>(`/dashboards/${slug}`, data),
  remove: (slug: string) => api.delete(`/dashboards/${slug}`),
  toggleStar: (slug: string) => api.post<Dashboard>(`/dashboards/${slug}/star`),
  listPanels: (slug: string) => api.get<Panel[]>(`/dashboards/${slug}/panels`),
  addPanel: (slug: string, data: Partial<Panel>) =>
    api.post<Panel>(`/dashboards/${slug}/panels`, data),
  updatePanel: (id: string, data: Partial<Panel>) => api.put<Panel>(`/panels/${id}`, data),
  removePanel: (id: string) => api.delete(`/panels/${id}`),
}
