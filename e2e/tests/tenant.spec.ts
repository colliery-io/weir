import { test, expect } from './fixtures';

// [[WEIR-T-0097]]: the tenant-aware UI. The fixture seeds the bootstrap admin key, so this user is a
// platform-admin → the switcher + the tenants admin panel are present.
test('admin: tenant switcher + tenants admin panel + re-scope', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByText('Run feed')).toBeVisible(); // authed

  // Admin controls are present.
  const switcher = page.locator('select.weir-tenant');
  await expect(switcher).toBeVisible();
  await expect(page.getByTitle('administer tenants')).toBeVisible();

  // Open the tenants panel + create a tenant.
  await page.getByTitle('administer tenants').click();
  await expect(page.getByText('administer tenants + their keys')).toBeVisible();
  await page.fill('input[placeholder="acme"]', 'acme');
  await page.getByRole('button', { name: 'Create tenant' }).click();
  await expect(page.locator('.weir-ttable', { hasText: 'acme' })).toBeVisible();

  // Mint a key for acme → the plaintext shows once.
  await page.getByText('keys →').first().click();
  await page.fill('input[placeholder="ci"]', 'ci-key');
  await page.getByRole('button', { name: 'Mint key' }).click();
  await expect(page.locator('.weir-minted')).toContainText('weirk_');

  // Close the panel, switch to acme → the page reloads scoped to acme.
  await page.getByText('✕ close').click();
  await switcher.selectOption('acme');
  await expect(page.getByText('Run feed')).toBeVisible(); // reloaded, still authed
  await expect(switcher).toHaveValue('acme'); // scope persisted across the reload
});
