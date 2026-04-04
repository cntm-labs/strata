import { describe, it, expect } from 'vitest'
import router from '../index'

describe('router', () => {
  it('redirects / to /dashboards', () => {
    const root = router.getRoutes().find((r) => r.path === '/')
    expect(root?.redirect).toBe('/dashboards')
  })

  const expectedRoutes = [
    '/dashboards',
    '/dashboards/new',
    '/dashboards/:slug',
    '/dashboards/:slug/edit',
    '/explore',
    '/alerts',
    '/alerts/rules/new',
    '/alerts/rules/:id',
    '/alerts/events',
    '/datasources',
    '/datasources/new',
    '/datasources/:id',
    '/templates',
    '/settings',
  ]

  for (const path of expectedRoutes) {
    it(`has route for ${path}`, () => {
      const route = router.getRoutes().find((r) => r.path === path)
      expect(route, `Route ${path} should exist`).toBeDefined()
    })
  }
})
