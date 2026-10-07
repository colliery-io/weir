import { test, expect } from './fixtures';

// [[WEIR-T-0214]]: the connection form renders one config form per side from each
// connector's `config_schema`, checks required fields before submit, merges each side's
// "Advanced JSON" over its fields, and POSTs `source_config` + `dest_config` (no shared
// `config`). Hermetic: the catalog, the two specs and the create are routed in the browser.

const srcSchema = {
  type: 'object',
  required: ['host', 'token'],
  properties: {
    host: { type: 'string' },
    port: { type: 'integer' },
    token: { type: 'string', airbyte_secret: true },
    mode: { type: 'string', enum: ['fast', 'safe'] },
    tls: { type: 'boolean' },
  },
};
const dstSchema = {
  type: 'object',
  required: ['bucket'],
  properties: { bucket: { type: 'string' }, region: { type: 'string' } },
};

test('config forms: two-sided schema forms post source_config + dest_config', async ({ page }) => {
  await page.route('**/catalog', (route) =>
    route.fulfill({
      json: [
        { name: 'cfg-src', version: '1.0.0', roles: ['Source'] },
        { name: 'cfg-dst', version: '1.0.0', roles: ['Destination'] },
      ],
    }),
  );
  await page.route('**/connectors/cfg-src/spec', (route) =>
    route.fulfill({ json: { config_schema: JSON.stringify(srcSchema) } }),
  );
  await page.route('**/connectors/cfg-dst/spec', (route) =>
    route.fulfill({ json: { config_schema: JSON.stringify(dstSchema) } }),
  );
  await page.route('**/connectors/cfg-src/discover', (route) => route.fulfill({ json: [] }));
  const posted: any[] = [];
  await page.route('**/connections', (route) => {
    if (route.request().method() !== 'POST') return route.fallback();
    posted.push(route.request().postDataJSON());
    return route.fulfill({ status: 201, json: {} });
  });

  await page.goto('/');
  await page.getByRole('button', { name: 'Setup' }).click();
  await page.getByPlaceholder('my-sync').fill('two-sided');
  await page.getByLabel('Source', { exact: true }).selectOption('cfg-src');
  await page.getByLabel('Destination', { exact: true }).selectOption('cfg-dst');

  // One field per schema property, on each side; required ones are marked.
  const source = page.getByTestId('config-source');
  const dest = page.getByTestId('config-destination');
  await expect(source.getByLabel('host *')).toBeVisible();
  await expect(source.getByLabel('port', { exact: true })).toBeVisible();
  await expect(source.getByLabel('token *')).toHaveAttribute('type', 'password');
  await expect(source.getByLabel('mode', { exact: true }).locator('option[value="safe"]')).toHaveCount(1);
  await expect(source.getByRole('switch', { name: 'tls' })).toBeVisible();
  await expect(dest.getByLabel('bucket *')).toBeVisible();

  // Required fields are checked before submit: nothing is sent.
  await page.getByRole('button', { name: 'Save connection' }).click();
  await expect(page.getByRole('alert').filter({ hasText: 'Source config: missing required host, token' })).toBeVisible();
  expect(posted).toHaveLength(0);

  await source.getByLabel('host *').fill('db.internal');
  await source.getByLabel('port', { exact: true }).fill('5432');
  await source.getByLabel('token *').fill('s3cret');
  await source.getByLabel('mode', { exact: true }).selectOption('safe');
  await source.getByRole('switch', { name: 'tls' }).click();
  await dest.getByLabel('bucket *').fill('lake');

  // Advanced JSON is collapsed until asked for; it is merged over the fields.
  await expect(dest.getByLabel(/Destination advanced JSON ·/)).toHaveCount(0);
  await dest.getByRole('switch', { name: 'Destination advanced JSON' }).click();
  await dest.getByLabel(/Destination advanced JSON ·/).fill('{"region": "eu-west-1", "bucket": "lake-2"}');

  await page.getByRole('button', { name: 'Save connection' }).click();
  await expect(page.getByText('Saved connection two-sided')).toBeVisible();
  expect(posted).toHaveLength(1);
  const body = posted[0];
  expect(body.source_config).toEqual({ host: 'db.internal', port: 5432, token: 's3cret', mode: 'safe', tls: true });
  expect(body.dest_config).toEqual({ bucket: 'lake-2', region: 'eu-west-1' });
  expect(body).not.toHaveProperty('config');
});
