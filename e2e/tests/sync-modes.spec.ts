import { test, expect } from './fixtures';
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
  else await field.fill(stream);
}

test('sync modes: an incremental upsert cron connection is created and read back', async ({
  page,
}) => {
  // Real create + GET against the e2e server (frankfurter + ArrowSink are seeded).
  const name = 'fx-modes';
  await page.request.delete(`/connections/${name}`, { headers: auth() });

  await page.goto('/');
  await page.getByRole('button', { name: 'Setup' }).click();
  await page.getByPlaceholder('my-sync').fill(name);
  await page.getByLabel('Source', { exact: true }).selectOption('frankfurter');
  await page.getByLabel('Destination', { exact: true }).selectOption('ArrowSink');
  await pickStream(page, 'latest');

  // Progressive disclosure: no cursor or keys until the mode needs them.
  await expect(page.getByLabel('Cursor field *')).toHaveCount(0);
  await expect(page.getByLabel('Business keys *')).toHaveCount(0);
  await page.getByLabel('Sync mode').selectOption('incremental');
  await page.getByLabel('Write mode').selectOption('upsert');
  await page.getByLabel('Cursor field *').fill('date');

  // Upsert without keys is stopped in the form, before any request.
  await page.getByRole('button', { name: 'Save connection' }).click();
  await expect(page.getByRole('alert').filter({ hasText: 'Upsert needs at least one business key' })).toBeVisible();
  await page.getByLabel('Business keys *').fill('base, date');

  // An interval typed first, then the toggle moved to cron: only the cron is sent.
  await page.getByLabel('Every (secs)').fill('60');
  await page.getByRole('button', { name: 'Cron', exact: true }).click();
  await page.getByLabel(/^Cron ·/).fill('0 0 3 * * *');

  await page.getByRole('button', { name: 'Save connection' }).click();
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
