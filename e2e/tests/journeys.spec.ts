import { test, expect, admin, scoped, ensurePair } from './fixtures';
import type { Page, Response } from '@playwright/test';

// [[WEIR-T-0220]]: the user journeys of COLLIERY-I-0251, end to end against the real e2e
// server (no routing): create a connection in the form, run it and see rows arrive; start and
// stop a resident source; the non-admin view; an admin's Setup in a switched tenant.
//
// The server's state outlives a spec (and a retry), and its seed step ignores errors: each
// journey makes the names, tenants, keys and catalog entries it needs. A create on this server
// is slow (it loads each side's wasm guest, synchronously, in a debug build), so the journeys
// wait on the real responses and poll the API, never on fixed sleeps or the 10s toast.

const uniq = (prefix: string) => `${prefix}-${Date.now().toString(36)}`;

/** The next response to `METHOD path` (exact pathname), with the slow-create budget. */
function nextResponse(page: Page, method: string, path: string, timeout = 90_000): Promise<Response> {
  return page.waitForResponse(
    (r) => r.request().method() === method && new URL(r.url()).pathname === path,
    { timeout },
  );
}

/** Fail with the server's reason when a UI request was refused (no silent failure). */
async function expectOk(res: Response) {
  const why = res.ok() ? '' : await res.text().catch(() => '');
  expect(res.ok(), `${res.request().method()} ${new URL(res.url()).pathname} → ${res.status()}: ${why}`).toBeTruthy();
}

/**
 * Do a UI action and wait for the request it sends; the request must succeed. The e2e server
 * is one sqlite file, and its API handlers do not retry a write that meets the background
 * worker's lock (the worker's own writes do): such a 500 "database is locked" is retried, as
 * the user would, a few times and noted on the test. Any other refusal fails with its reason.
 */
async function act(page: Page, method: string, path: string, click: () => Promise<void>): Promise<Response> {
  for (let attempt = 1; ; attempt++) {
    const answered = nextResponse(page, method, path);
    await click();
    const res = await answered;
    if (res.ok() || attempt === 3) {
      await expectOk(res);
      return res;
    }
    const why = await res.text().catch(() => '');
    if (!why.includes('database is locked')) await expectOk(res);
    test.info().annotations.push({ type: 'sqlite-lock', description: `${method} ${path} try ${attempt}: ${why}` });
    await page.waitForTimeout(1_000);
  }
}

/** Fill the Setup form: frankfurter `latest` → the arrow sink. `tenant` scopes the discover. */
async function fillPair(page: Page, name: string, tenant: string | null) {
  await page.getByPlaceholder('my-sync').fill(name);
  // Picking the source rediscovers its streams (a guest call); let it settle first. Its
  // status is not checked here: on main, discovery of a manifest source answers 500 and the
  // form falls back to a text stream field (the discovery fix is PR #21's own work).
  const discovered = nextResponse(page, 'POST', scoped(tenant, '/connectors/frankfurter/discover'));
  await page.getByLabel('Source', { exact: true }).selectOption('frankfurter');
  await discovered;
  const dest = page.getByLabel('Destination', { exact: true });
  const arrow = dest.locator('option').filter({ hasText: /arrow/i }).first();
  await expect(arrow).toBeAttached();
  await dest.selectOption((await arrow.getAttribute('value'))!);
  // The stream is a select when discovery returned streams, else a text input.
  const stream = page.getByLabel('Stream', { exact: true });
  if ((await stream.evaluate((e) => e.tagName)) === 'SELECT') await stream.selectOption('latest');
  else await stream.fill('latest');
}

/** Save the form and wait for the real create; fail with the server's reason. */
async function saveForm(page: Page, name: string, tenant: string | null) {
  await act(page, 'POST', scoped(tenant, '/connections'), () =>
    page.getByRole('button', { name: 'Save connection' }).click(),
  );
  await expect(page.getByText(`Saved connection ${name}`)).toBeVisible();
}

const card = (page: Page, name: string) => page.getByTestId('connection-card').filter({ hasText: name });

test('journey: create in the form → run → rows arrive', async ({ page }) => {
  test.setTimeout(180_000);
  const name = uniq('fx-journey');
  await ensurePair(page.request);

  await page.goto('/');
  await page.getByRole('button', { name: 'Setup' }).click();
  await fillPair(page, name, null);
  await saveForm(page, name, null);

  // Run it from its card.
  await page.getByRole('button', { name: 'Operations' }).click();
  await expect(card(page, name)).toBeVisible({ timeout: 30_000 });
  const res = await act(page, 'POST', `/connections/${name}/run`, () =>
    card(page, name).getByRole('button', { name: 'Run', exact: true }).click(),
  );
  // The id is a snowflake past 2^53: read it from the text, not through a JS number.
  const id = /"work_unit_id"\s*:\s*(\d+)/.exec(await res.text())?.[1];
  expect(id, 'the run response names its unit').toBeTruthy();

  // The worker runs it against the live frankfurter API: rows arrive in the arrow sink.
  let run: any = null;
  await expect
    .poll(
      async () => {
        const r = await page.request.get(`/runs/${id}`, { headers: admin() });
        run = r.ok() ? await r.json() : { state: `HTTP ${r.status()}` };
        return run.state;
      },
      { timeout: 120_000, intervals: [1_000] },
    )
    .toMatch(/^(done|failed)$/);
  expect(run.state, `run ${id} failed: ${run.error}`).toBe('done');
  expect(run.rows_written, 'rows written').toBeGreaterThan(0);

  // The run detail in the UI shows the same count.
  const row = page.getByRole('row', { name: new RegExp(`^run \\d+ · ${name}$`) }).first();
  await expect(row).toBeVisible({ timeout: 30_000 });
  await row.click();
  const dialog = page.getByRole('dialog', { name: /^Run #\d+$/ });
  await expect(dialog).toBeVisible();
  const rows = dialog.locator('.cl-kv', { has: page.locator('dt', { hasText: /^rows written$/ }) }).locator('dd');
  await expect(rows).toHaveText(String(run.rows_written));

  await page.request.delete(`/connections/${name}`, { headers: admin() });
});

test('journey: a resident source starts and stops from its card', async ({ page }) => {
  test.setTimeout(180_000);
  const name = uniq('fx-resident');
  const dest = await ensurePair(page.request);
  const body = {
    name,
    source: 'frankfurter',
    dest,
    stream: 'latest',
    source_config: {},
    dest_config: {},
    every_secs: 30,
    execution_mode: 'resident',
  };
  let created = await page.request.post('/connections', { headers: admin(), timeout: 90_000, data: body });
  for (let i = 0; i < 2 && (await created.text()).includes('database is locked'); i++) {
    created = await page.request.post('/connections', { headers: admin(), timeout: 90_000, data: body });
  }
  expect(created.ok(), `create ${name}: ${created.status()} ${await created.text()}`).toBeTruthy();
  const active = async () =>
    ((await (await page.request.get(`/connections/${name}/runs`, { headers: admin() })).json()) as { state: string }[])
      .filter((u) => u.state === 'pending' || u.state === 'leased').length;

  await page.goto('/');
  const c = card(page, name);
  await expect(c).toBeVisible({ timeout: 30_000 });
  // A resident card has Start/Stop, no Run.
  await expect(c.getByRole('button', { name: 'Run', exact: true })).toHaveCount(0);
  await expect(c.getByText('resident • stopped')).toBeVisible();

  await act(page, 'POST', `/connections/${name}/start`, () =>
    c.getByRole('button', { name: 'Start', exact: true }).click(),
  );
  await expect(page.getByText(`Started ${name}`)).toBeVisible();
  await expect(c.getByText('resident • live')).toBeVisible({ timeout: 30_000 });
  await expect.poll(active, { timeout: 30_000 }).toBe(1);

  await act(page, 'POST', `/connections/${name}/stop`, () =>
    c.getByRole('button', { name: 'Stop', exact: true }).click(),
  );
  await expect(page.getByText(`Stopped ${name}`)).toBeVisible();
  await expect(c.getByText('resident • stopped')).toBeVisible({ timeout: 30_000 });
  // Durably stopped: the worker does not bring it back.
  await expect.poll(active, { timeout: 30_000 }).toBe(0);
  await page.waitForTimeout(3_000);
  expect(await active(), 'no unit came back after the stop').toBe(0);
  await expect(c.getByText('resident • stopped')).toBeVisible();

  await page.request.delete(`/connections/${name}`, { headers: admin() });
});

test('journey: a non-admin key gets a tenant chip, no switcher, no Platform', async ({ page }) => {
  const tenant = uniq('na');
  const made = await page.request.post('/tenants', { headers: admin(), data: { id: tenant } });
  expect(made.ok(), `create tenant ${tenant}: ${made.status()} ${await made.text()}`).toBeTruthy();
  const minted = await page.request.post(`/tenants/${tenant}/keys`, {
    headers: admin(),
    data: { name: 'e2e-writer', role: 'write' },
  });
  expect(minted.ok(), `mint key: ${minted.status()} ${await minted.text()}`).toBeTruthy();
  const key: string = (await minted.json()).key;
  expect(key).toMatch(/^weirk_/);

  // Sign in as the non-admin (this init script runs after the fixture's admin one).
  await page.addInitScript((k) => localStorage.setItem('weir_api_key', k), key);
  const me = page.waitForResponse((r) => new URL(r.url()).pathname === '/auth/me');
  await page.goto('/');
  expect(await (await me).json()).toMatchObject({ tenant, is_admin: false });
  await expect(page.getByText('Run feed')).toBeVisible();

  // The tenant shows as a chip, not a switcher; no Platform tab, no tenants admin.
  await expect(page.getByText(`⊙ ${tenant}`, { exact: true })).toBeVisible();
  await expect(page.getByRole('combobox', { name: 'Tenant' })).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Setup' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Platform' })).toHaveCount(0);
  await expect(page.getByTitle('administer tenants')).toHaveCount(0);

  await page.request.delete(`/tenants/${tenant}`, { headers: admin() });
});

test('journey: an admin switched to another tenant onboards and creates there', async ({ page }) => {
  test.setTimeout(180_000);
  const tenant = uniq('tb');
  const name = uniq('fx-tb');
  const made = await page.request.post('/tenants', { headers: admin(), data: { id: tenant } });
  expect(made.ok(), `create tenant ${tenant}: ${made.status()} ${await made.text()}`).toBeTruthy();

  // The admin has the switcher and the Platform tab; switch to tenant B.
  await page.goto('/');
  await expect(page.getByRole('button', { name: 'Platform' })).toBeVisible();
  const switcher = page.getByRole('combobox', { name: 'Tenant' });
  await expect(switcher.locator(`option[value="${tenant}"]`)).toHaveCount(1);
  await switcher.selectOption(tenant);
  await expect(page.getByText('Run feed')).toBeVisible();
  await expect(switcher).toHaveValue(tenant);

  // Onboard the pair into B's own catalog from the picker.
  await page.getByRole('button', { name: 'Setup' }).click();
  const picker = page.getByLabel('Pick a connector');
  for (const pkg of ['frankfurter', 'weir-arrow-sink-pkg']) {
    await expect(picker.locator(`option[value="${pkg}"]`)).toHaveCount(1);
    await picker.selectOption(pkg);
    await act(page, 'POST', `/tenants/${tenant}/catalog/import`, () =>
      page.getByRole('button', { name: 'Onboard' }).first().click(),
    );
    await expect(page.getByText(`Onboarded ${pkg}`)).toBeVisible();
  }

  // Create the connection in B through the form.
  await fillPair(page, name, tenant);
  await saveForm(page, name, tenant);

  // It is in B, and not in the admin's own tenant.
  const inB = await page.request.get(`/tenants/${tenant}/connections`, { headers: admin() });
  expect(inB.ok()).toBeTruthy();
  expect(((await inB.json()) as { name: string }[]).map((c) => c.name)).toContain(name);
  const own = await page.request.get('/connections', { headers: admin() });
  expect(((await own.json()) as { name: string }[]).map((c) => c.name)).not.toContain(name);
  // And B's Operations lists it.
  await page.getByRole('button', { name: 'Operations' }).click();
  await expect(card(page, name)).toBeVisible({ timeout: 30_000 });

  await page.request.delete(`/tenants/${tenant}`, { headers: admin() });
});
