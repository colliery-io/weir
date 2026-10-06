import { test, expect } from './fixtures';

// [[WEIR-T-0097]]: the tenant-aware UI. The fixture seeds the bootstrap admin key, so this user is a
// platform-admin → the switcher + the tenants admin panel are present.
test('admin: tenant switcher + tenants admin panel + re-scope', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByText('Run feed')).toBeVisible(); // authed

  // Admin controls are present.
  const switcher = page.getByRole('combobox', { name: 'Tenant' });
  await expect(switcher).toBeVisible();
  await expect(page.getByTitle('administer tenants')).toBeVisible();

  // Open the tenants panel + create a tenant.
  await page.getByTitle('administer tenants').click();
  const dialog = page.getByRole('dialog', { name: 'Tenants' });
  await expect(dialog.getByText('administer tenants + their keys')).toBeVisible();
  await page.fill('input[placeholder="acme"]', 'acme');
  await page.getByRole('button', { name: 'Create tenant' }).click();
  await expect(dialog.getByRole('table', { name: 'Tenants' })).toContainText('acme');

  // Mint a key for acme → the plaintext shows once (Aurora SecretReveal).
  await dialog.getByRole('row', { name: 'keys of acme' }).click();
  await page.fill('input[placeholder="ci"]', 'ci-key');
  await page.getByRole('button', { name: 'Mint key' }).click();
  await expect(dialog.locator('code', { hasText: 'weirk_' })).toBeVisible();

  // Close the panel, switch to acme → the page reloads scoped to acme.
  await dialog.getByRole('button', { name: 'Close', exact: true }).click();
  await expect(dialog).toHaveCount(0);
  await switcher.selectOption('acme');
  await expect(page.getByText('Run feed')).toBeVisible(); // reloaded, still authed
  await expect(switcher).toHaveValue('acme'); // scope persisted across the reload
});
