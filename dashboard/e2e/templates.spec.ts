import { test, expect } from '@playwright/test'

test.describe('Templates', () => {
  test('page shows heading', async ({ page }) => {
    await page.goto('/templates')
    await expect(page.getByRole('heading', { name: /Templates/i })).toBeVisible()
  })
})
