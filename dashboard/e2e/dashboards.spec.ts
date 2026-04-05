import { test, expect } from '@playwright/test'

test.describe('Dashboards', () => {
  test('list page shows heading and new button', async ({ page }) => {
    await page.goto('/dashboards')
    await expect(page.getByRole('heading', { name: 'Dashboards' })).toBeVisible()
    await expect(page.getByRole('link', { name: /New Dashboard/i })).toBeVisible()
  })

  test('new dashboard link navigates to form', async ({ page }) => {
    await page.goto('/dashboards')
    await page.getByRole('link', { name: /New Dashboard/i }).click()
    await expect(page).toHaveURL(/\/dashboards\/new/)
  })
})
