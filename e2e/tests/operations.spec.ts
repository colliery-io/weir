import { test, expect } from './fixtures';

// [[WEIR-T-0080]]: clicking a connection card opens the Aurora Modal run-detail
// (requires a seeded `fx-demo` connection on the running server).
test('operations: card opens the run-detail modal', async ({ page }) => {
  await page.goto('/');
  await expect(page.locator('.weir-card__name', { hasText: 'fx-demo' })).toBeVisible();

  await page.locator('.weir-card__name').first().click();
  await expect(page.getByText('run detail')).toBeVisible();
  // Lineage panel ([[WEIR-T-0101]]): the source→dest chain renders.
  await expect(page.getByText('Lineage')).toBeVisible();
  await expect(page.getByText('Dead-letters')).toBeVisible();
  await expect(page.getByText('Logs').first()).toBeVisible();
});
