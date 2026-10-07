import { test, expect } from './fixtures';

// [[WEIR-T-0218]]: stream discovery reads the chosen source and its source-side config
// (fields + Advanced JSON). It waits for the required fields, re-runs (debounced) when the
// config changes, shows a failure next to the stream field, can be re-run by hand, and
// drops an older answer that arrives after a newer request. Hermetic: catalog, spec and
// discover are routed in the browser.

const schema = {
  type: 'object',
  required: ['token'],
  properties: { token: { type: 'string', airbyte_secret: true }, region: { type: 'string' } },
};

test('discovery: credentials filled after the source is chosen bring the streams', async ({ page }) => {
  await page.route('**/catalog', (route) =>
    route.fulfill({
      json: [
        { name: 'd-src', version: '1.0.0', roles: ['Source'] },
        { name: 'd-dst', version: '1.0.0', roles: ['Destination'] },
      ],
    }),
  );
  await page.route('**/connectors/d-src/spec', (route) =>
    route.fulfill({ json: { config_schema: JSON.stringify(schema) } }),
  );
  await page.route('**/connectors/d-dst/spec', (route) => route.fulfill({ json: {} }));
  const bodies: any[] = [];
  await page.route('**/connectors/d-src/discover', async (route) => {
    const body = route.request().postDataJSON();
    bodies.push(body);
    if (body.token === 'slow') {
      // Answers after the next request has been answered: a stale response.
      await new Promise((r) => setTimeout(r, 2500));
      return route.fulfill({ json: ['stale_stream'] });
    }
    if (body.token !== 'good') {
      return route.fulfill({ status: 400, json: { error: 'bad token' } });
    }
    return route.fulfill({ json: body.region ? ['orders', `orders_${body.region}`] : ['orders', 'customers'] });
  });

  await page.goto('/');
  await page.getByRole('button', { name: 'Setup' }).click();
  await page.getByLabel('Source', { exact: true }).selectOption('d-src');

  // Nothing is sent while the required credential is missing.
  const status = page.getByTestId('stream-discovery');
  await expect(status).toContainText('missing required token');
  expect(bodies).toHaveLength(0);
  await expect(page.getByLabel('Stream', { exact: true })).toHaveJSProperty('tagName', 'INPUT');

  // A wrong credential: the server's reason is shown next to the stream field.
  const source = page.getByTestId('config-source');
  await source.getByLabel('token *').fill('nope');
  await expect(status).toContainText('Stream discovery failed: bad token');
  await expect(status).toHaveAttribute('data-error', 'true');

  // The right one: the streams appear.
  await source.getByLabel('token *').fill('good');
  const stream = page.getByLabel('Stream', { exact: true });
  await expect(stream.locator('option[value="customers"]')).toHaveCount(1);
  await expect(status).toContainText('2 streams discovered');
  expect(bodies.at(-1)).toEqual({ token: 'good' });

  // Advanced JSON is part of the source config: it re-runs discovery too.
  await source.getByRole('switch', { name: 'Source advanced JSON' }).click();
  await source.getByLabel(/Source advanced JSON ·/).fill('{"region": "eu"}');
  await expect(stream.locator('option[value="orders_eu"]')).toHaveCount(1);
  expect(bodies.at(-1)).toEqual({ token: 'good', region: 'eu' });

  // The manual re-run sends the same config again.
  const before = bodies.length;
  await page.getByRole('button', { name: 'Refresh streams' }).click();
  await expect.poll(() => bodies.length).toBe(before + 1);
  expect(bodies.at(-1)).toEqual({ token: 'good', region: 'eu' });

  // A slow answer that lands after a newer one is ignored.
  await source.getByLabel(/Source advanced JSON ·/).fill('');
  await source.getByLabel('token *').fill('slow');
  await expect.poll(() => bodies.at(-1)?.token).toBe('slow');
  await source.getByLabel('token *').fill('good');
  await expect(stream.locator('option[value="customers"]')).toHaveCount(1);
  await page.waitForTimeout(3000); // past the slow answer
  await expect(stream.locator('option[value="stale_stream"]')).toHaveCount(0);
  await expect(stream.locator('option[value="customers"]')).toHaveCount(1);
});

test('discovery: an edited connection is discovered from its loaded config, stored secrets withheld', async ({
  page,
}) => {
  const SENTINEL = '__weir_secret_unchanged__';
  const editSchema = {
    type: 'object',
    required: ['host'],
    properties: { host: { type: 'string' }, token: { type: 'string', airbyte_secret: true } },
  };
  await page.route('**/catalog', (route) =>
    route.fulfill({
      json: [
        { name: 'l-src', version: '1.0.0', roles: ['Source'] },
        { name: 'l-dst', version: '1.0.0', roles: ['Destination'] },
      ],
    }),
  );
  await page.route('**/connectors/l-src/spec', (route) =>
    route.fulfill({ json: { config_schema: JSON.stringify(editSchema) } }),
  );
  await page.route('**/connectors/l-dst/spec', (route) => route.fulfill({ json: {} }));
  const bodies: any[] = [];
  await page.route('**/connectors/l-src/discover', (route) => {
    bodies.push(route.request().postDataJSON());
    return route.fulfill({ json: ['orders', 'refunds'] });
  });
  await page.route('**/connections/loaded-sync/schema', (route) =>
    route.fulfill({ json: { fields: [], broken: null } }),
  );
  await page.route('**/connections/loaded-sync', (route) =>
    route.fulfill({
      json: {
        name: 'loaded-sync',
        source: 'l-src',
        dest: 'l-dst',
        stream: 'refunds',
        source_config: { host: 'db.internal', token: SENTINEL },
        dest_config: {},
        every_secs: null,
        cron: null,
        sync_mode: 'full_refresh',
        write_mode: 'append',
        business_keys: [],
        cursor_field: null,
        execution_mode: 'run_once',
      },
    }),
  );
  await page.route('**/connections', (route) =>
    route.fulfill({
      json: [{ name: 'loaded-sync', source: 'l-src', dest: 'l-dst', stream: 'refunds', execution_mode: 'run_once' }],
    }),
  );

  await page.goto('/');
  const card = page.getByTestId('connection-card').filter({ hasText: 'loaded-sync' });
  await card.getByRole('button', { name: 'Edit' }).click();
  await expect(page.getByPlaceholder('my-sync')).toHaveValue('loaded-sync');

  // The loaded config is kept (not wiped by the connector change) and discovered;
  // the stored secret is not sent.
  await expect(page.getByTestId('config-source').getByLabel('host *')).toHaveValue('db.internal');
  const stream = page.getByLabel('Stream', { exact: true });
  await expect(stream.locator('option[value="refunds"]')).toHaveCount(1);
  await expect(stream).toHaveValue('refunds');
  expect(bodies.at(-1)).toEqual({ host: 'db.internal' });
});
