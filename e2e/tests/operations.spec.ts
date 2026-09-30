import { test, expect } from './fixtures';

// [[WEIR-T-0080]]: clicking a connection card opens the Aurora Modal run-detail
// (requires a seeded `fx-demo` connection on the running server).
test('operations: card opens the run-detail modal', async ({ page }) => {
  await page.goto('/');
  const card = page.getByTestId('connection-card').filter({ hasText: 'fx-demo' });
  await expect(card).toBeVisible();

  await card.first().click();
  await expect(page.getByRole('dialog', { name: 'Run detail' })).toBeVisible();
  // Lineage panel ([[WEIR-T-0101]]): the source→dest chain renders.
  const dialog = page.getByRole('dialog', { name: 'Run detail' });
  await expect(dialog.getByRole('heading', { name: 'Lineage' })).toBeVisible();
  await expect(dialog.getByRole('heading', { name: 'Dead-letters' })).toBeVisible();
  await expect(dialog.getByRole('heading', { name: 'Logs' })).toBeVisible();
});
