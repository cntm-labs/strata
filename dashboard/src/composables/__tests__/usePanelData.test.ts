import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'

vi.mock('@/api/datasources', () => ({
  datasourcesApi: {
    query: vi.fn(),
  },
}))

import { usePanelData } from '../usePanelData'
import { datasourcesApi } from '@/api/datasources'

const mockPanel = {
  id: '1',
  dashboard_id: 'd1',
  title: 'Test',
  type: 'stat' as const,
  datasource_id: 'ds-1',
  query: 'up',
  config: {},
  position: { x: 0, y: 0, w: 3, h: 2, i: '1' },
  created_at: '',
  updated_at: '',
}

beforeEach(() => {
  vi.clearAllMocks()
  vi.useFakeTimers()
  vi.mocked(datasourcesApi.query).mockResolvedValue({ data: 'result' })
})

afterEach(() => {
  vi.useRealTimers()
})

describe('usePanelData', () => {
  it('fetches data on creation', async () => {
    usePanelData(mockPanel, '1h', 0)

    await vi.waitFor(() => {
      expect(datasourcesApi.query).toHaveBeenCalledTimes(1)
    })

    expect(datasourcesApi.query).toHaveBeenCalledWith(
      'ds-1',
      expect.objectContaining({
        query: 'up',
      }),
    )
  })

  it('skips fetch when no datasource_id', () => {
    const panel = { ...mockPanel, datasource_id: undefined }
    usePanelData(panel, '1h', 0)
    expect(datasourcesApi.query).not.toHaveBeenCalled()
  })

  it('calculates correct time range for 5m', async () => {
    const now = 1700000000
    vi.setSystemTime(now * 1000)

    usePanelData(mockPanel, '5m', 0)

    await vi.waitFor(() => {
      expect(datasourcesApi.query).toHaveBeenCalled()
    })

    const call = vi.mocked(datasourcesApi.query).mock.calls[0]
    expect(call[1].start).toBe((now - 300).toString())
    expect(call[1].end).toBe(now.toString())
  })

  it('falls back to 1h for unknown range', async () => {
    const now = 1700000000
    vi.setSystemTime(now * 1000)

    usePanelData(mockPanel, 'unknown', 0)

    await vi.waitFor(() => {
      expect(datasourcesApi.query).toHaveBeenCalled()
    })

    const call = vi.mocked(datasourcesApi.query).mock.calls[0]
    expect(call[1].start).toBe((now - 3600).toString())
  })

  it('exposes refresh function', () => {
    const { refresh } = usePanelData(mockPanel, '1h', 0)
    expect(typeof refresh).toBe('function')
  })
})
