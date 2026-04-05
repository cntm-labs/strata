import { test, expect } from '@playwright/test'

test.describe('Navigation', () => {
  test('root redirects to /dashboards', async ({ page }) => {
    await page.goto('/')
    await expect(page).toHaveURL(/\/dashboards/)
  })

  test('sidebar links navigate correctly', async ({ page }) => {
    await page.goto('/dashboards')

    const navLinks = [
      { label: 'Dashboards', url: '/dashboards' },
      { label: 'Explore', url: '/explore' },
      { label: 'Alerts', url: '/alerts' },
      { label: 'Data Sources', url: '/datasources' },
      { label: 'Templates', url: '/templates' },
      { label: 'Settings', url: '/settings' },
    ]

    for (const link of navLinks) {
      await page.getByRole('link', { name: link.label }).click()
      await expect(page).toHaveURL(new RegExp(link.url))
    }
  })

  test('shows Strata branding in sidebar', async ({ page }) => {
    await page.goto('/dashboards')
    await expect(page.locator('aside')).toContainText('Strata')
  })
})
