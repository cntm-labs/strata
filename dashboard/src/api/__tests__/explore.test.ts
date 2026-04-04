import { describe, it, expect, vi, beforeEach } from 'vitest'

vi.mock('../client', () => ({
  api: {
    get: vi.fn().mockResolvedValue([]),
    post: vi.fn().mockResolvedValue({}),
  },
}))

import { exploreApi } from '../explore'
import { api } from '../client'

beforeEach(() => {
  vi.clearAllMocks()
})

describe('exploreApi', () => {
  it('query calls POST /explore/query', async () => {
    const data = { datasource_id: 'ds1', query: 'up' }
    await exploreApi.query(data)
    expect(api.post).toHaveBeenCalledWith('/explore/query', data)
  })

  it('query with all params', async () => {
    const data = { datasource_id: 'ds1', query: 'up', start: '1000', end: '2000', step: '15' }
    await exploreApi.query(data)
    expect(api.post).toHaveBeenCalledWith('/explore/query', data)
  })

  it('history without params', async () => {
    await exploreApi.history()
    expect(api.get).toHaveBeenCalledWith('/explore/history')
  })

  it('history with datasource_id filter', async () => {
    await exploreApi.history({ datasource_id: 'ds1' })
    expect(api.get).toHaveBeenCalledWith('/explore/history?datasource_id=ds1')
  })

  it('history with limit', async () => {
    await exploreApi.history({ limit: 10 })
    expect(api.get).toHaveBeenCalledWith('/explore/history?limit=10')
  })

  it('labels calls GET /explore/labels/:id', async () => {
    await exploreApi.labels('ds1')
    expect(api.get).toHaveBeenCalledWith('/explore/labels/ds1')
  })
})
