import { describe, it, expect, vi, beforeEach } from 'vitest'

vi.mock('../client', () => ({
  api: {
    get: vi.fn().mockResolvedValue([]),
    post: vi.fn().mockResolvedValue({}),
    put: vi.fn().mockResolvedValue({}),
    delete: vi.fn().mockResolvedValue(null),
  },
}))

import { datasourcesApi } from '../datasources'
import { api } from '../client'

beforeEach(() => {
  vi.clearAllMocks()
})

describe('datasourcesApi', () => {
  it('list calls GET /datasources', async () => {
    await datasourcesApi.list()
    expect(api.get).toHaveBeenCalledWith('/datasources')
  })

  it('get calls GET /datasources/:id', async () => {
    await datasourcesApi.get('abc')
    expect(api.get).toHaveBeenCalledWith('/datasources/abc')
  })

  it('create calls POST /datasources', async () => {
    const data = { name: 'Prom', type: 'prometheus' as const, url: 'http://prom:9090' }
    await datasourcesApi.create(data)
    expect(api.post).toHaveBeenCalledWith('/datasources', data)
  })

  it('update calls PUT /datasources/:id', async () => {
    const data = { name: 'Updated' }
    await datasourcesApi.update('abc', data)
    expect(api.put).toHaveBeenCalledWith('/datasources/abc', data)
  })

  it('remove calls DELETE /datasources/:id', async () => {
    await datasourcesApi.remove('abc')
    expect(api.delete).toHaveBeenCalledWith('/datasources/abc')
  })

  it('test calls POST /datasources/:id/test', async () => {
    await datasourcesApi.test('abc')
    expect(api.post).toHaveBeenCalledWith('/datasources/abc/test')
  })

  it('query calls POST /datasources/:id/query', async () => {
    const data = { query: 'up', start: '1000', end: '2000', step: '15' }
    await datasourcesApi.query('abc', data)
    expect(api.post).toHaveBeenCalledWith('/datasources/abc/query', data)
  })
})
