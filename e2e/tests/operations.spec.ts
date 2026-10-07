import { test, expect } from './fixtures';

// [[WEIR-T-0080]]: clicking a connection card opens the Aurora Modal connection detail
// (requires a seeded `fx-demo` connection on the running server).
test('operations: card opens the connection-detail modal', async ({ page }) => {
  await page.goto('/');
  const card = page.getByTestId('connection-card').filter({ hasText: 'fx-demo' });
  await expect(card).toBeVisible();

  await card.first().click();
  await expect(page.getByRole('dialog', { name: 'Connection detail' })).toBeVisible();
  // Lineage panel ([[WEIR-T-0101]]): the source→dest chain renders.
  const dialog = page.getByRole('dialog', { name: 'Connection detail' });
  await expect(dialog.getByRole('heading', { name: 'Lineage' })).toBeVisible();
  await expect(dialog.getByRole('heading', { name: 'Dead-letters' })).toBeVisible();
  await expect(dialog.getByRole('heading', { name: 'Logs' })).toBeVisible();
});

// [[WEIR-T-0219]]: a run-feed row opens that run's detail (GET /runs/{id}), and the feed
// pages back through history (GET /runs?before=…). The seeded `fx-demo` run is enough.
test('operations: a run-feed row opens the run detail; the feed pages', async ({ page }) => {
  await page.goto('/');
  const row = page.getByRole('row', { name: /^run \d+ · fx-demo$/ }).first();
  await expect(row).toBeVisible();
  await row.click();

  const dialog = page.getByRole('dialog', { name: /^Run #\d+$/ });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByText('fx-demo', { exact: true }).first()).toBeVisible();
  await expect(dialog.getByText('started', { exact: true })).toBeVisible();
  await expect(dialog.getByText('duration', { exact: true })).toBeVisible();
  await expect(dialog.getByRole('heading', { name: 'Committed state · connection' })).toBeVisible();
  await expect(dialog.getByText('committed cursor', { exact: true })).toBeVisible();
  await expect(dialog.getByRole('heading', { name: 'Dead-letters · connection' })).toBeVisible();
  await expect(dialog.getByRole('heading', { name: 'Logs · connection' })).toBeVisible();

  // From the run to its connection.
  await dialog.getByRole('button', { name: 'Open connection' }).click();
  await expect(page.getByRole('dialog', { name: 'Connection detail' })).toBeVisible();
  await page.keyboard.press('Escape');

  // One seeded run: the first older page is empty, so the feed says so.
  await page.getByRole('button', { name: 'Load older runs' }).click();
  await expect(page.getByText('No older runs.')).toBeVisible();
});
