import { test, expect } from '@playwright/test'

test.describe('Data Sources', () => {
  test('list page shows heading', async ({ page }) => {
    await page.goto('/datasources')
    await expect(page.getByRole('heading', { name: /Data Sources/i })).toBeVisible()
  })
})
