import { test, expect, act, api, ensurePair, expectOk, fillStable } from './fixtures';

// [[WEIR-T-0216]]: Edit on a connection card loads it into the Setup form. Secrets come
// back from the server as the sentinel ([[WEIR-T-0201]]); the form shows them as
// "unchanged" and sends the sentinel back, unless the user types a new value or clears it.

const SENTINEL = '__weir_secret_unchanged__';

test('edit connection: a non-secret edit keeps the stored secret (real server)', async ({ page }) => {
  test.setTimeout(120_000);
  // A fresh name every time: the server's state outlives a spec (and a retry).
  const name = `fx-edit-${Date.now().toString(36)}`;
  await ensurePair(page.request);
  // `api_key` is always a secret key (the baked auth metadata rule).
  const created = await api(page.request, 'POST', '/connections', {
    data: {
      name,
      source: 'frankfurter',
      dest: 'ArrowSink',
      stream: 'latest',
      source_config: { api_key: 's3cret-e2e' },
      dest_config: {},
    },
  });
  await expectOk(created, `create ${name}`);

  await page.goto('/');
  const card = page.getByTestId('connection-card').filter({ hasText: name });
  await card.getByRole('button', { name: 'Edit' }).click();

  // The form holds the connection; its name is read-only.
  const nameInput = page.getByPlaceholder('my-sync');
  await expect(nameInput).toHaveValue(name);
  await expect(nameInput).toBeDisabled();
  await expect(page.getByText(`Editing ${name}`)).toBeVisible();

  // Edit a non-secret field only.
  await fillStable(page.getByLabel('Every (secs)'), '300');
  // Wait on the save itself (a create is slow on the e2e server), and check what it sent.
  const res = await act(page, 'POST', '/connections', () =>
    page.getByRole('button', { name: 'Save connection' }).click(),
  );
  expect(res.request().postDataJSON()).toMatchObject({ name, every_secs: 300 });
  await expect(page.getByText(`Saved connection ${name}`)).toBeVisible();
  // Edit mode ends with the save.
  await expect(page.getByPlaceholder('my-sync')).toBeEnabled();

  const got = await api(page.request, 'GET', `/connections/${name}`);
  await expectOk(got);
  const c = await got.json();
  expect(c.every_secs).toBe(300);
  expect(c.stream).toBe('latest');
  // The server still holds a non-empty secret: a read redacts it to the sentinel
  // (a cleared or lost secret would read back as "" or be absent).
  expect(c.source_config.api_key).toBe(SENTINEL);

  await api(page.request, 'DELETE', `/connections/${name}`);
});

const srcSchema = {
  type: 'object',
  required: ['host'],
  properties: {
    host: { type: 'string' },
    token: { type: 'string', airbyte_secret: true },
  },
};

test('edit connection: stored secrets are kept, replaced or cleared (hermetic)', async ({ page }) => {
  await page.route('**/catalog', (route) =>
    route.fulfill({
      json: [
        { name: 'e-src', version: '1.0.0', roles: ['Source'] },
        { name: 'e-dst', version: '1.0.0', roles: ['Destination'] },
      ],
    }),
  );
  await page.route('**/connectors/e-src/spec', (route) =>
    route.fulfill({ json: { config_schema: JSON.stringify(srcSchema) } }),
  );
  await page.route('**/connectors/e-dst/spec', (route) => route.fulfill({ json: {} }));
  await page.route('**/connectors/e-src/discover', (route) => route.fulfill({ json: ['orders'] }));
  await page.route('**/connections/sec-sync/schema', (route) =>
    route.fulfill({ json: { fields: [], broken: null } }),
  );
  await page.route('**/connections/sec-sync', (route) =>
    route.fulfill({
      json: {
        name: 'sec-sync',
        source: 'e-src',
        dest: 'e-dst',
        stream: 'orders',
        source_config: { host: 'db.internal', token: SENTINEL },
        dest_config: { region: 'eu' },
        every_secs: 60,
        cron: null,
        sync_mode: 'full_refresh',
        write_mode: 'upsert',
        business_keys: ['id', 'region'],
        cursor_field: null,
        execution_mode: 'run_once',
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
      json: [{ name: 'sec-sync', source: 'e-src', dest: 'e-dst', stream: 'orders', execution_mode: 'run_once' }],
    });
  });

  const openEdit = async () => {
    await page.getByRole('button', { name: 'Operations' }).click();
    const card = page.getByTestId('connection-card').filter({ hasText: 'sec-sync' });
    await card.getByRole('button', { name: 'Edit' }).click();
    await expect(page.getByPlaceholder('my-sync')).toHaveValue('sec-sync');
  };
  const save = async (n: number) => {
    await page.getByRole('button', { name: 'Save connection' }).click();
    await expect.poll(() => posted.length).toBe(n);
  };

  await page.goto('/');
  await openEdit();

  // Every field of the form is loaded.
  const source = page.getByTestId('config-source');
  await expect(page.getByPlaceholder('my-sync')).toBeDisabled();
  await expect(page.getByLabel('Source', { exact: true })).toHaveValue('e-src');
  await expect(page.getByLabel('Destination', { exact: true })).toHaveValue('e-dst');
  await expect(page.getByLabel('Write mode')).toHaveValue('upsert');
  await expect(page.getByLabel('Business keys *')).toHaveValue('id, region');
  await expect(page.getByLabel('Every (secs)')).toHaveValue('60');
  await expect(source.getByLabel('host *')).toHaveValue('db.internal');
  // The secret is masked as "unchanged": the sentinel is never shown.
  const token = source.getByTestId('secret-token');
  await expect(token.getByLabel('token', { exact: true })).toHaveValue('');
  await expect(token.getByText('unchanged · the stored value is kept')).toBeVisible();

  // 1. A non-secret edit sends the sentinel back for the secret.
  await source.getByLabel('host *').fill('db2.internal');
  await save(1);
  expect(posted[0]).toMatchObject({
    name: 'sec-sync',
    source: 'e-src',
    dest: 'e-dst',
    stream: 'orders',
    source_config: { host: 'db2.internal', token: SENTINEL },
    dest_config: { region: 'eu' },
    write_mode: 'upsert',
    business_keys: ['id', 'region'],
    every_secs: 60,
  });

  // 2. "Keep stored" undoes a clear: the sentinel goes back.
  await openEdit();
  const secret = page.getByTestId('secret-token');
  await secret.getByRole('button', { name: 'Clear' }).click();
  await expect(secret.getByText('will be cleared on save')).toBeVisible();
  await secret.getByRole('button', { name: 'Keep stored' }).click();
  await save(2);
  expect(posted[1].source_config.token).toBe(SENTINEL);

  // 3. Clear sends "", which clears the stored secret.
  await openEdit();
  await page.getByTestId('secret-token').getByRole('button', { name: 'Clear' }).click();
  await save(3);
  expect(posted[2].source_config.token).toBe('');

  // 4. A typed value replaces it.
  await openEdit();
  await page.getByTestId('secret-token').getByLabel('token', { exact: true }).fill('n3w-s3cret');
  await save(4);
  expect(posted[3].source_config.token).toBe('n3w-s3cret');
});
