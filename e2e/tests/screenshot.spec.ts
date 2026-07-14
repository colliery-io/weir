import { test } from './fixtures';

// Capture the two views so the Aurora Dark re-skin can be eyeballed.
test('capture UI screenshots for theme review', async ({ page }) => {
  await page.setViewportSize({ width: 1360, height: 940 });
  await page.goto('/');
  await page.waitForTimeout(900);
  await page.screenshot({ path: 'shots/operations.png', fullPage: true });

  await page.getByRole('button', { name: 'Setup' }).click();
  await page.waitForTimeout(500);
  await page.screenshot({ path: 'shots/setup.png', fullPage: true });
});
