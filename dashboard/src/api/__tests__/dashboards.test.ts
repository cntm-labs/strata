import { describe, it, expect, vi, beforeEach } from 'vitest'

vi.mock('../client', () => ({
  api: {
    get: vi.fn().mockResolvedValue([]),
    post: vi.fn().mockResolvedValue({}),
    put: vi.fn().mockResolvedValue({}),
    delete: vi.fn().mockResolvedValue(null),
  },
}))

import { dashboardsApi } from '../dashboards'
import { api } from '../client'

beforeEach(() => {
  vi.clearAllMocks()
})

describe('dashboardsApi', () => {
  it('list calls GET /dashboards', async () => {
    await dashboardsApi.list()
    expect(api.get).toHaveBeenCalledWith('/dashboards')
  })

  it('get calls GET /dashboards/:slug', async () => {
    await dashboardsApi.get('my-dash')
    expect(api.get).toHaveBeenCalledWith('/dashboards/my-dash')
  })

  it('create calls POST /dashboards', async () => {
    const data = { title: 'New' }
    await dashboardsApi.create(data)
    expect(api.post).toHaveBeenCalledWith('/dashboards', data)
  })

  it('update calls PUT /dashboards/:slug', async () => {
    const data = { title: 'Updated' }
    await dashboardsApi.update('my-dash', data)
    expect(api.put).toHaveBeenCalledWith('/dashboards/my-dash', data)
  })

  it('remove calls DELETE /dashboards/:slug', async () => {
    await dashboardsApi.remove('my-dash')
    expect(api.delete).toHaveBeenCalledWith('/dashboards/my-dash')
  })

  it('toggleStar calls POST /dashboards/:slug/star', async () => {
    await dashboardsApi.toggleStar('my-dash')
    expect(api.post).toHaveBeenCalledWith('/dashboards/my-dash/star')
  })

  it('listPanels calls GET /dashboards/:slug/panels', async () => {
    await dashboardsApi.listPanels('my-dash')
    expect(api.get).toHaveBeenCalledWith('/dashboards/my-dash/panels')
  })

  it('addPanel calls POST /dashboards/:slug/panels', async () => {
    const data = { title: 'Panel', type: 'stat' as const }
    await dashboardsApi.addPanel('my-dash', data)
    expect(api.post).toHaveBeenCalledWith('/dashboards/my-dash/panels', data)
  })

  it('updatePanel calls PUT /panels/:id', async () => {
    const data = { title: 'Updated' }
    await dashboardsApi.updatePanel('abc', data)
    expect(api.put).toHaveBeenCalledWith('/panels/abc', data)
  })

  it('removePanel calls DELETE /panels/:id', async () => {
    await dashboardsApi.removePanel('abc')
    expect(api.delete).toHaveBeenCalledWith('/panels/abc')
  })
})
