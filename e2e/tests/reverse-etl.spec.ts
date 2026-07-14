import { test, expect } from './fixtures';

// [[WEIR-T-0077]] verification: the piece a Ralph loop couldn't self-check.
// Drives the real embedded Leptos + Aurora UI ([[WEIR-I-0016]]): a destination manifest
// is discoverable, onboards, and then appears in a connection's *destination* dropdown.
test('destination manifest: discover → onboard → appears in the dest dropdown', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Setup' }).click();

  // The "Add a connector" picker offers the HubSpot *destination*, labelled "· destination".
  const picker = page.locator('label.weir-fld').filter({ hasText: 'pick a connector' }).locator('select');
  const hubspotOption = picker.locator('option', { hasText: 'hubspot-dest' });
  await expect(hubspotOption).toHaveText(/destination/);

  // Onboard it (option value is the package name — distinct from the source `hubspot`).
  await picker.selectOption('hubspot-dest');
  // The "Add a connector" panel's Onboard button (the "Bring your own" panel has one too).
  await page.getByRole('button', { name: 'Onboard' }).first().click();
  await expect(page.getByText('Onboarded hubspot-dest')).toBeVisible();

  // Onboarded → dropped from the discover picker, and now a selectable option exactly once:
  // the **destination** dropdown (the source dropdown excludes it — Destination-only role).
  await expect(picker.locator('option[value="hubspot-dest"]')).toHaveCount(0);
  await expect(page.locator('option[value="hubspot-dest"]')).toHaveCount(1);

  // And it is genuinely selectable as the connection's destination.
  const destSelect = page.locator('select', { has: page.locator('option[value="hubspot-dest"]') });
  await destSelect.selectOption('hubspot-dest');
  await expect(destSelect).toHaveValue('hubspot-dest');
});
