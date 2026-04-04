import { api } from './client'
import type { Datasource } from '@/types'

export const datasourcesApi = {
  list: () => api.get<Datasource[]>('/datasources'),
  get: (id: string) => api.get<Datasource>(`/datasources/${id}`),
  create: (data: Partial<Datasource>) => api.post<Datasource>('/datasources', data),
  update: (id: string, data: Partial<Datasource>) =>
    api.put<Datasource>(`/datasources/${id}`, data),
  remove: (id: string) => api.delete(`/datasources/${id}`),
  test: (id: string) => api.post<{ success: boolean }>(`/datasources/${id}/test`),
  query: (id: string, data: { query: string; start?: string; end?: string; step?: string }) =>
    api.post(`/datasources/${id}/query`, data),
}
