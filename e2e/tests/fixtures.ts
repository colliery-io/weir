import { test as base, expect } from '@playwright/test';
import type { APIRequestContext, APIResponse, Locator, Page, Response } from '@playwright/test';

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

// The e2e server is one sqlite file. Its API handlers do not retry a write that meets the
// background worker's lock (the worker's own writes do), so any call can answer
// 5xx "database is locked". That is the test bed, not the journey under test: such an answer
// is retried a few times (and noted on the test); any other refusal fails with its reason.
const LOCKED = 'database is locked';
const note = (what: string) => test.info().annotations.push({ type: 'sqlite-lock', description: what });

/** A direct API call, retried on a sqlite lock. The caller checks the final answer. */
export async function api(
  request: APIRequestContext,
  method: 'GET' | 'POST' | 'DELETE',
  url: string,
  options: { headers?: Record<string, string>; data?: unknown; timeout?: number } = {},
): Promise<APIResponse> {
  const opts = { headers: admin(), timeout: 90_000, ...options };
  for (let attempt = 1; ; attempt++) {
    const res = await request.fetch(url, { method, ...opts });
    if (res.ok() || attempt === 5) return res;
    const why = await res.text().catch(() => '');
    if (!why.includes(LOCKED)) return res;
    note(`${method} ${url} try ${attempt}: ${why}`);
    await new Promise((r) => setTimeout(r, 500 * attempt));
  }
}

/** Fail with the server's reason when a request was refused (no silent failure). */
export async function expectOk(res: Response | APIResponse, what?: string) {
  const why = res.ok() ? '' : await res.text().catch(() => '');
  const label = what ?? ('request' in res ? `${res.request().method()} ${new URL(res.url()).pathname}` : res.url());
  expect(res.ok(), `${label} → ${res.status()}: ${why}`).toBeTruthy();
}

/** The next response to `METHOD path` (exact pathname), with the slow-create budget. */
export function nextResponse(page: Page, method: string, path: string, timeout = 90_000): Promise<Response> {
  return page.waitForResponse(
    (r) => r.request().method() === method && new URL(r.url()).pathname === path,
    { timeout },
  );
}

/**
 * Do a UI action and wait for the request it sends; the request must succeed. A sqlite-lock
 * answer is retried as the user would, by doing the action again.
 */
export async function act(page: Page, method: string, path: string, click: () => Promise<void>): Promise<Response> {
  for (let attempt = 1; ; attempt++) {
    const answered = nextResponse(page, method, path);
    await click();
    const res = await answered;
    if (res.ok() || attempt === 5) {
      await expectOk(res);
      return res;
    }
    const why = await res.text().catch(() => '');
    if (!why.includes(LOCKED)) await expectOk(res);
    note(`${method} ${path} try ${attempt}: ${why}`);
    await page.waitForTimeout(500 * attempt);
  }
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
    ((await (await api(request, 'GET', scoped(tenant, '/catalog'))).json()) as { name: string }[]).map(
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
      await api(request, 'POST', scoped(tenant, '/catalog/import'), { data: body });
    }
  }
  const have = await names();
  expect(have, 'catalog has frankfurter').toContain('frankfurter');
  expect(have.some(isArrow), `catalog has the arrow sink: ${have}`).toBeTruthy();
  return have.find(isArrow)!;
}
