import { api } from './client'
import type { AlertRule, AlertEvent } from '@/types'

export const alertsApi = {
  listRules: () => api.get<AlertRule[]>('/alerts/rules'),
  getRule: (id: string) => api.get<AlertRule>(`/alerts/rules/${id}`),
  createRule: (data: Partial<AlertRule>) => api.post<AlertRule>('/alerts/rules', data),
  updateRule: (id: string, data: Partial<AlertRule>) =>
    api.put<AlertRule>(`/alerts/rules/${id}`, data),
  deleteRule: (id: string) => api.delete(`/alerts/rules/${id}`),
  listEvents: (params?: { rule_id?: string; limit?: number }) => {
    const qs = new URLSearchParams()
    if (params?.rule_id) qs.set('rule_id', params.rule_id)
    if (params?.limit) qs.set('limit', params.limit.toString())
    const query = qs.toString()
    return api.get<AlertEvent[]>(`/alerts/events${query ? `?${query}` : ''}`)
  },
}
