import { describe, it, expect, vi, beforeEach } from 'vitest'

vi.mock('../client', () => ({
  api: {
    get: vi.fn().mockResolvedValue([]),
    post: vi.fn().mockResolvedValue({}),
  },
}))

import { templatesApi } from '../templates'
import { api } from '../client'

beforeEach(() => {
  vi.clearAllMocks()
})

describe('templatesApi', () => {
  it('list calls GET /templates', async () => {
    await templatesApi.list()
    expect(api.get).toHaveBeenCalledWith('/templates')
  })

  it('use calls POST /templates/:slug/use', async () => {
    const data = { title: 'My Dash', slug: 'my-dash' }
    await templatesApi.use('node-exporter', data)
    expect(api.post).toHaveBeenCalledWith('/templates/node-exporter/use', data)
  })

  it('use with datasource_id', async () => {
    const data = { title: 'My Dash', slug: 'my-dash', datasource_id: 'ds1' }
    await templatesApi.use('node-exporter', data)
    expect(api.post).toHaveBeenCalledWith('/templates/node-exporter/use', data)
  })
})
