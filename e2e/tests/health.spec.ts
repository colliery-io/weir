import { test, expect } from './fixtures';

// [[WEIR-T-0111]]: the per-tenant health dashboard — a "needs attention" summary + a status grid
// of connections coloured by health, backed by GET /overview. Uses the seeded `fx-demo` connection.
test('health: per-connection dashboard renders', async ({ page }) => {
  await page.goto('/');

  // Switch to the Health view (a segmented-control option).
  await page.getByText('Health', { exact: true }).click();

  // Both panels render.
  await expect(page.getByText('Needs attention')).toBeVisible();
  await expect(page.getByText('Connection health')).toBeVisible();

  // The seeded connection appears as a health card with its freshness/error stats.
  const card = page.locator('.weir-health-card', { hasText: 'fx-demo' });
  await expect(card).toBeVisible();
  await expect(card).toContainText('lag');
  await expect(card).toContainText('dl');
});

// [[WEIR-T-0112]]: the super-operator cross-tenant view — admin-only. The Platform segment shows
// per-tenant health rollups; the seeded server's `default` tenant appears.
test('platform: super-operator cross-tenant view (admin)', async ({ page }) => {
  await page.goto('/');

  // The admin-only Platform segment is present (the e2e key is an admin key).
  await page.getByText('Platform', { exact: true }).click();

  // The default tenant appears in the cross-tenant rollup as a health card.
  await expect(page.locator('.weir-health-name', { hasText: 'default' })).toBeVisible();
});

// [[WEIR-T-0121]]: the connection detail surfaces the captured typed schema (+ a drift banner when a
// breaking change is flagged). Opens the seeded `fx-demo` connection from the Health grid.
test('schema: connection detail shows the typed schema section', async ({ page }) => {
  await page.goto('/');
  await page.getByText('Health', { exact: true }).click();

  const card = page.locator('.weir-health-card', { hasText: 'fx-demo' });
  await expect(card).toBeVisible();
  await card.click();

  // The detail modal renders the Schema section (captured fields, or the empty state).
  await expect(page.locator('.weir-schema-view')).toBeVisible();
});
