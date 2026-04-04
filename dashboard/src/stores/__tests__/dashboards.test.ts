import { describe, it, expect, vi, beforeEach } from 'vitest'
import { setActivePinia, createPinia } from 'pinia'

vi.mock('@/api/dashboards', () => ({
  dashboardsApi: {
    list: vi.fn(),
  },
}))

import { useDashboardStore } from '../dashboards'
import { dashboardsApi } from '@/api/dashboards'

beforeEach(() => {
  setActivePinia(createPinia())
  vi.clearAllMocks()
})

describe('useDashboardStore', () => {
  it('initializes with empty items and loading false', () => {
    const store = useDashboardStore()
    expect(store.items).toEqual([])
    expect(store.loading).toBe(false)
  })

  it('fetchAll sets items and manages loading state', async () => {
    const mockDashboards = [{ id: '1', title: 'Test', slug: 'test' }]
    vi.mocked(dashboardsApi.list).mockResolvedValueOnce(
      mockDashboards as unknown as Awaited<ReturnType<typeof dashboardsApi.list>>,
    )

    const store = useDashboardStore()
    await store.fetchAll()

    expect(store.items).toEqual(mockDashboards)
    expect(store.loading).toBe(false)
  })

  it('fetchAll resets loading on error', async () => {
    vi.mocked(dashboardsApi.list).mockRejectedValueOnce(new Error('fail'))

    const store = useDashboardStore()
    await expect(store.fetchAll()).rejects.toThrow('fail')
    expect(store.loading).toBe(false)
  })
})
