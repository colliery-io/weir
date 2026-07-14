import { test, expect } from './fixtures';

// [[WEIR-T-0079]] smoke: the Leptos + Aurora shell loads and the Operations/Setup
// SegmentedControl toggles the body panel.
test('leptos aurora shell loads + tabs toggle', async ({ page }) => {
  await page.goto('/');

  // Default view is Operations (the Run feed panel is Operations-only).
  await expect(page.getByText('Run feed')).toBeVisible();

  // Toggle to Setup.
  await page.getByRole('button', { name: 'Setup' }).click();
  await expect(page.getByText('Run feed')).toHaveCount(0);

  // Back to Operations.
  await page.getByRole('button', { name: 'Operations' }).click();
  await expect(page.getByText('Run feed')).toBeVisible();
});
