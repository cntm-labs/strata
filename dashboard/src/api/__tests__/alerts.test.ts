import { describe, it, expect, vi, beforeEach } from 'vitest'

vi.mock('../client', () => ({
  api: {
    get: vi.fn().mockResolvedValue([]),
    post: vi.fn().mockResolvedValue({}),
    put: vi.fn().mockResolvedValue({}),
    delete: vi.fn().mockResolvedValue(null),
  },
}))

import { alertsApi } from '../alerts'
import { api } from '../client'

beforeEach(() => {
  vi.clearAllMocks()
})

describe('alertsApi', () => {
  it('listRules calls GET /alerts/rules', async () => {
    await alertsApi.listRules()
    expect(api.get).toHaveBeenCalledWith('/alerts/rules')
  })

  it('getRule calls GET /alerts/rules/:id', async () => {
    await alertsApi.getRule('abc')
    expect(api.get).toHaveBeenCalledWith('/alerts/rules/abc')
  })

  it('createRule calls POST /alerts/rules', async () => {
    const data = { name: 'CPU Alert' }
    await alertsApi.createRule(data)
    expect(api.post).toHaveBeenCalledWith('/alerts/rules', data)
  })

  it('updateRule calls PUT /alerts/rules/:id', async () => {
    const data = { name: 'Updated' }
    await alertsApi.updateRule('abc', data)
    expect(api.put).toHaveBeenCalledWith('/alerts/rules/abc', data)
  })

  it('deleteRule calls DELETE /alerts/rules/:id', async () => {
    await alertsApi.deleteRule('abc')
    expect(api.delete).toHaveBeenCalledWith('/alerts/rules/abc')
  })

  it('listEvents without params calls GET /alerts/events', async () => {
    await alertsApi.listEvents()
    expect(api.get).toHaveBeenCalledWith('/alerts/events')
  })

  it('listEvents with rule_id filter', async () => {
    await alertsApi.listEvents({ rule_id: 'abc' })
    expect(api.get).toHaveBeenCalledWith('/alerts/events?rule_id=abc')
  })

  it('listEvents with rule_id and limit', async () => {
    await alertsApi.listEvents({ rule_id: 'abc', limit: 10 })
    expect(api.get).toHaveBeenCalledWith('/alerts/events?rule_id=abc&limit=10')
  })

  it('listEvents with limit only', async () => {
    await alertsApi.listEvents({ limit: 50 })
    expect(api.get).toHaveBeenCalledWith('/alerts/events?limit=50')
  })
})
