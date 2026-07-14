import { test, expect } from '@playwright/test'; // raw: authenticate via the OIDC flow, not a seeded key

// [[WEIR-T-0088]]: the human door end-to-end — sign in through Dex, land back authenticated.
test.skip(!process.env.WEIR_OIDC_ISSUER, 'needs Dex + OIDC env');

test('OIDC sign-in via Dex → authenticated app', async ({ page }) => {
  test.skip(!process.env.WEIR_OIDC_ISSUER, 'needs Dex + the OIDC env (see WEIR-T-0088)');
  await page.goto('/');
  await expect(page.getByText('Sign in to weir')).toBeVisible();
  await page.getByRole('button', { name: 'Sign in with OIDC' }).click();

  // Dex static-password login form.
  await page.waitForURL(/:5557\/dex/, { timeout: 15000 });
  await page.fill('input[name="login"]', 'admin@weir.test');
  await page.fill('input[name="password"]', 'password');
  await page.click('button[type="submit"]');

  // Back at weir, the session cookie authenticates → the app (Run feed) shows.
  await expect(page.getByText('Run feed')).toBeVisible({ timeout: 15000 });
});
