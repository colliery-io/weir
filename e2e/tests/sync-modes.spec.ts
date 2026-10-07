import { test, expect, ensurePair, fillStable } from './fixtures';
import type { Page } from '@playwright/test';

// [[WEIR-T-0215]]: the connection form sets sync mode, write mode, business keys, the
// cursor field and a schedule (every N seconds OR a cron expression — exactly one is sent).

const auth = () => {
  const key = process.env.WEIR_E2E_KEY;
  return key ? { Authorization: `Bearer ${key}` } : undefined;
};

// The stream is a select when discovery returns streams, else a text input.
async function pickStream(page: Page, stream: string) {
  const field = page.getByLabel('Stream', { exact: true });
  if ((await field.evaluate((e) => e.tagName)) === 'SELECT') await field.selectOption(stream);
  else await fillStable(field, stream);
}

test('sync modes: an incremental upsert cron connection is created and read back', async ({
  page,
}) => {
  // Real create + GET against the e2e server (frankfurter + the arrow sink are seeded).
  // The create is slow on that server: to validate the config it loads each side's
  // connector (a wasm guest, in a debug build) several times, synchronously — seen over
  // 10s on a CI runner, with the server's other requests queued behind it. The budget is
  // sized for that work.
  test.setTimeout(120_000);
  // The server's state outlives a spec (and a retry): a fresh name every time.
  const name = `fx-modes-${Date.now().toString(36)}`;
  // The harness seed ignores errors (a locked-db import has left the arrow sink out on CI).
  await ensurePair(page.request);

  await page.goto('/');
  await page.getByRole('button', { name: 'Setup' }).click();
  await fillStable(page.getByPlaceholder('my-sync'), name);
  // Picking the source rediscovers its streams (a guest call on the server); wait for that
  // answer, so the stream field is settled (text input or select) before it is picked.
  const discovered = page.waitForResponse(
    (r) => r.request().method() === 'POST' && r.url().endsWith('/connectors/frankfurter/discover'),
  );
  await page.getByLabel('Source', { exact: true }).selectOption('frankfurter');
  await discovered;
  // The seeded arrow sink's catalog name is the package's, not the `ArrowSink` alias.
  const dest = page.getByLabel('Destination', { exact: true });
  const arrow = dest.locator('option').filter({ hasText: /arrow/i }).first();
  await expect(arrow).toBeAttached();
  await dest.selectOption((await arrow.getAttribute('value'))!);
  await pickStream(page, 'latest');

  // Progressive disclosure: no cursor or keys until the mode needs them.
  await expect(page.getByLabel('Cursor field *')).toHaveCount(0);
  await expect(page.getByLabel('Business keys *')).toHaveCount(0);
  await page.getByLabel('Sync mode').selectOption('incremental');
  await page.getByLabel('Write mode').selectOption('upsert');
  await fillStable(page.getByLabel('Cursor field *'), 'date');

  // Upsert without keys is stopped in the form, before any request.
  await page.getByRole('button', { name: 'Save connection' }).click();
  await expect(page.getByRole('alert').filter({ hasText: 'Upsert needs at least one business key' })).toBeVisible();
  await fillStable(page.getByLabel('Business keys *'), 'base, date');

  // An interval typed first, then the toggle moved to cron: only the cron is sent.
  await fillStable(page.getByLabel('Every (secs)'), '60');
  await page.getByRole('button', { name: 'Cron', exact: true }).click();
  await fillStable(page.getByLabel(/^Cron ·/), '0 0 3 * * *');

  // Wait on the create itself, not on its toast with the default 10s: the toast shows
  // only once the (slow, see above) create answers, and a refused create fails here
  // with the server's reason.
  const created = page.waitForResponse(
    (r) => r.request().method() === 'POST' && new URL(r.url()).pathname === '/connections',
    { timeout: 90_000 },
  );
  await page.getByRole('button', { name: 'Save connection' }).click();
  const res = await created;
  // A 201 has no body (and Chromium keeps none to read): the reason is read on failure only.
  const why = res.ok() ? '' : await res.text().catch(() => '');
  expect(res.ok(), `POST /connections ${res.status()}: ${why}`).toBeTruthy();
  expect(res.request().postDataJSON()).toMatchObject({ name, cron: '0 0 3 * * *', every_secs: null });
  await expect(page.getByText(`Saved connection ${name}`)).toBeVisible();

  const got = await page.request.get(`/connections/${name}`, { headers: auth() });
  expect(got.ok()).toBeTruthy();
  const c = await got.json();
  expect(c.sync_mode).toBe('incremental');
  expect(c.write_mode).toBe('upsert');
  expect(c.cursor_field).toBe('date');
  expect(c.business_keys).toEqual(['base', 'date']);
  expect(c.cron).toBe('0 0 3 * * *');
  expect(c.every_secs).toBeNull();

  await page.request.delete(`/connections/${name}`, { headers: auth() });
});

test('sync modes: the cursor field is a select from the captured schema', async ({ page }) => {
  // Hermetic: an existing connection with a captured schema, and the create, are routed.
  await page.route('**/catalog', (route) =>
    route.fulfill({
      json: [
        { name: 'm-src', version: '1.0.0', roles: ['Source'] },
        { name: 'm-dst', version: '1.0.0', roles: ['Destination'] },
      ],
    }),
  );
  await page.route('**/connectors/*/spec', (route) => route.fulfill({ json: {} }));
  await page.route('**/connectors/m-src/discover', (route) => route.fulfill({ json: ['orders'] }));
  await page.route('**/connections/orders-sync/schema', (route) =>
    route.fulfill({
      json: {
        fields: [
          { name: 'id', type: 'Int64', nullable: false },
          { name: 'updated_at', type: 'Timestamp', nullable: true },
        ],
        broken: null,
      },
    }),
  );
  const posted: any[] = [];
  await page.route('**/connections', (route) => {
    if (route.request().method() === 'POST') {
      posted.push(route.request().postDataJSON());
      return route.fulfill({ status: 201, json: {} });
    }
    return route.fulfill({
      json: [{ name: 'orders-sync', source: 'm-src', dest: 'm-dst', stream: 'orders', execution_mode: 'run_once' }],
    });
  });

  await page.goto('/');
  await page.getByRole('button', { name: 'Setup' }).click();
  await page.getByPlaceholder('my-sync').fill('orders-sync');
  await page.getByLabel('Source', { exact: true }).selectOption('m-src');
  await page.getByLabel('Destination', { exact: true }).selectOption('m-dst');
  await page.getByLabel('Stream', { exact: true }).selectOption('orders');
  await page.getByLabel('Sync mode').selectOption('incremental');

  const cursor = page.getByLabel('Cursor field *');
  await expect(cursor.locator('option[value="updated_at"]')).toHaveCount(1);
  await cursor.selectOption('updated_at');

  // A bad cron is stopped in the form; an interval is sent alone.
  await page.getByRole('button', { name: 'Cron', exact: true }).click();
  await page.getByLabel(/^Cron ·/).fill('0 * * * *');
  await page.getByRole('button', { name: 'Save connection' }).click();
  await expect(page.getByRole('alert').filter({ hasText: 'Cron needs 6 or 7 fields' })).toBeVisible();
  expect(posted).toHaveLength(0);
  await page.getByRole('button', { name: 'Every N seconds', exact: true }).click();
  await page.getByLabel('Every (secs)').fill('300');

  await page.getByRole('button', { name: 'Save connection' }).click();
  await expect(page.getByText('Saved connection orders-sync')).toBeVisible();
  expect(posted).toHaveLength(1);
  expect(posted[0]).toMatchObject({
    sync_mode: 'incremental',
    write_mode: 'append',
    cursor_field: 'updated_at',
    business_keys: [],
    every_secs: 300,
    cron: null,
  });
});
