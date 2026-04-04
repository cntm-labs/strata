import { describe, it, expect, vi, beforeEach } from 'vitest'
import { setActivePinia, createPinia } from 'pinia'

vi.mock('@/api/datasources', () => ({
  datasourcesApi: {
    list: vi.fn(),
  },
}))

import { useDatasourceStore } from '../datasources'
import { datasourcesApi } from '@/api/datasources'

beforeEach(() => {
  setActivePinia(createPinia())
  vi.clearAllMocks()
})

describe('useDatasourceStore', () => {
  it('initializes with empty items and loading false', () => {
    const store = useDatasourceStore()
    expect(store.items).toEqual([])
    expect(store.loading).toBe(false)
  })

  it('fetchAll sets items and manages loading state', async () => {
    const mockDatasources = [{ id: '1', name: 'Prom', type: 'prometheus' }]
    vi.mocked(datasourcesApi.list).mockResolvedValueOnce(
      mockDatasources as unknown as Awaited<ReturnType<typeof datasourcesApi.list>>,
    )

    const store = useDatasourceStore()
    await store.fetchAll()

    expect(store.items).toEqual(mockDatasources)
    expect(store.loading).toBe(false)
  })

  it('fetchAll resets loading on error', async () => {
    vi.mocked(datasourcesApi.list).mockRejectedValueOnce(new Error('fail'))

    const store = useDatasourceStore()
    await expect(store.fetchAll()).rejects.toThrow('fail')
    expect(store.loading).toBe(false)
  })
})
