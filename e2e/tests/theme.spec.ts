import { test, expect } from './fixtures';

// [[COLLIERY-T-1838]]: Aurora light/dark. The top bar has the Aurora ThemeToggle; the choice
// sets data-theme on <html>, is stored, and THEME_INIT_SCRIPT applies it on the next load.
test('theme: the top-bar toggle switches light/dark and survives a reload', async ({ page }) => {
  await page.goto('/');
  const toggle = page.getByRole('group', { name: 'Theme' });
  await expect(toggle).toBeVisible();

  await toggle.getByRole('button', { name: 'Light' }).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');

  await toggle.getByRole('button', { name: 'Dark' }).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');

  await page.reload();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await expect(toggle.getByRole('button', { name: 'Dark' })).toHaveAttribute('aria-pressed', 'true');

  // System: no forced theme; the page follows the OS again.
  await toggle.getByRole('button', { name: 'System' }).click();
  await expect(page.locator('html')).not.toHaveAttribute('data-theme', /light|dark/);
});
