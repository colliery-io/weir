import { test as base, expect } from '@playwright/test';
import type { APIRequestContext, Locator } from '@playwright/test';

// Seed the API key into localStorage before each test so the WEIR-T-0087 auth gate
// passes (the UI sends it as `Authorization: Bearer`). The server-start harness mints
// a key and exports it as WEIR_E2E_KEY.
export const test = base.extend({
  page: async ({ page }, use) => {
    const key = process.env.WEIR_E2E_KEY;
    if (key) {
      await page.addInitScript((k) => localStorage.setItem('weir_api_key', k as string), key);
    }
    await use(page);
  },
});

export { expect } from '@playwright/test';

/**
 * Fill a form field and make sure the value stuck. The Setup view is rebuilt when its data
 * lands (e.g. `/catalog/available`); a rebuild in the middle of a fill drops the typed text
 * with no error (seen on CI: `edit-connection` saved `every_secs: null` after a fill of 300).
 * So fill again until the field holds the value.
 */
export async function fillStable(field: Locator, value: string) {
  await expect(async () => {
    await field.fill(value);
    await expect(field).toHaveValue(value, { timeout: 1_000 });
  }).toPass({ timeout: 15_000 });
}

/** The admin key's bearer header for direct API calls. */
export const admin = () => {
  const key = process.env.WEIR_E2E_KEY;
  return key ? { Authorization: `Bearer ${key}` } : undefined;
};

/** `/tenants/{t}{path}` for a tenant, `path` for the key's own scope. */
export const scoped = (tenant: string | null, path: string) => (tenant ? `/tenants/${tenant}${path}` : path);

/**
 * Make sure frankfurter (source) and the arrow sink (destination) are in a tenant's catalog
 * ([[WEIR-T-0220]]). The harness seeds them, but its seed ignores errors (a "database is
 * locked" import has left the sink out on CI), so a spec that needs the pair asks for it.
 * Returns the arrow sink's catalog name.
 */
export async function ensurePair(request: APIRequestContext, tenant: string | null = null): Promise<string> {
  const names = async () =>
    ((await (await request.get(scoped(tenant, '/catalog'), { headers: admin() })).json()) as { name: string }[]).map(
      (c) => c.name,
    );
  const isArrow = (n: string) => /arrow/i.test(n);
  const want: [(n: string) => boolean, object][] = [
    [(n) => n === 'frankfurter', { manifest_name: 'frankfurter' }],
    [isArrow, { package: 'weir-arrow-sink-pkg' }],
  ];
  for (let attempt = 0; attempt < 5; attempt++) {
    const have = await names();
    const missing = want.filter(([is]) => !have.some(is));
    if (missing.length === 0) return have.find(isArrow)!;
    for (const [, body] of missing) {
      await request.post(scoped(tenant, '/catalog/import'), { headers: admin(), data: body, timeout: 90_000 });
    }
  }
  const have = await names();
  expect(have, 'catalog has frankfurter').toContain('frankfurter');
  expect(have.some(isArrow), `catalog has the arrow sink: ${have}`).toBeTruthy();
  return have.find(isArrow)!;
}
