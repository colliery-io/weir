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

  // Aurora toasts: an error toast is role=alert.
  const toast = page.getByRole('alert').filter({ hasText: "Couldn't save" });
  await expect(toast).toBeVisible();
  await expect(toast).toContainText("Couldn't save typo-proof");
  await expect(toast).toContainText('unknown source connector');
});

test('errors: unreachable control plane shows the degraded banner, not empty states', async ({
  page,
}) => {
  await page.goto('/');
  // Healthy first: the seeded fx-demo card renders.
  const card = page.getByTestId('connection-card').filter({ hasText: 'fx-demo' });
  await expect(card).toBeVisible();

  // Kill the API from the browser's point of view: the canary poll starts failing.
  await page.route('**/connections', (route) => route.abort());
  const banner = page.getByTestId('api-error');
  await expect(banner).toBeVisible({ timeout: 10_000 });
  await expect(banner).toContainText('control plane error');
  // Last-known data still shows — never a fake "No connections yet".
  await expect(card).toBeVisible();

  // Recovery: unblock the route → the banner clears on the next successful poll.
  await page.unroute('**/connections');
  await expect(banner).toHaveCount(0, { timeout: 15_000 });
});
