import { test, expect } from './fixtures';

// [[WEIR-T-0167]] (+ [[WEIR-T-0166]]): failures surface as errors — a rejected create
// carries the server's reason into the toast, and an unreachable control plane shows
// the degraded banner over last-known data instead of a fake empty dashboard.

test('errors: rejected create surfaces the server reason in the toast', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Setup' }).click();

  // Name only — no source selected, so the server rejects the create with the
  // [[WEIR-T-0166]] validation message (unknown connector, catalog pointer).
  await page.getByPlaceholder('my-sync').fill('typo-proof');
  await page.getByRole('button', { name: 'Save connection' }).click();

  const toast = page.locator('.weir-toast');
  await expect(toast).toBeVisible();
  await expect(toast).toContainText("Couldn't save typo-proof");
  await expect(toast).toContainText('unknown source connector');
});

test('errors: unreachable control plane shows the degraded banner, not empty states', async ({
  page,
}) => {
  await page.goto('/');
  // Healthy first: the seeded fx-demo card renders.
  await expect(page.locator('.weir-card__name', { hasText: 'fx-demo' })).toBeVisible();

  // Kill the API from the browser's point of view: the canary poll starts failing.
  await page.route('**/connections', (route) => route.abort());
  const banner = page.locator('.weir-apierr');
  await expect(banner).toBeVisible({ timeout: 10_000 });
  await expect(banner).toContainText('control plane error');
  // Last-known data still shows — never a fake "No connections yet".
  await expect(page.locator('.weir-card__name', { hasText: 'fx-demo' })).toBeVisible();

  // Recovery: unblock the route → the banner clears on the next successful poll.
  await page.unroute('**/connections');
  await expect(banner).toHaveCount(0, { timeout: 15_000 });
});
