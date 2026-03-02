import { expect, test } from '@playwright/test'

test('renders initial UI and increments counter', async ({ page }) => {
  await page.goto('/Reversi/')

  await expect(page.getByRole('heading', { name: 'Vite + React' })).toBeVisible()
  const counterButton = page.getByRole('button', { name: 'count is 0' })
  await expect(counterButton).toBeVisible()

  await counterButton.click()

  await expect(page.getByRole('button', { name: 'count is 1' })).toBeVisible()
})
