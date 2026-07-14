import { test, expect } from '@playwright/test'; // raw: no key seeding → the gate should show

test('sign-in gate shows when unauthenticated', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByText('Sign in to weir')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Sign in with OIDC' })).toBeVisible();
  await page.screenshot({ path: 'shots/gate.png' });
});
