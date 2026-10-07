import {
  test,
  expect,
  act,
  api,
  ensurePair,
  expectOk,
  fillStable,
  nextResponse,
  scoped,
} from './fixtures';
import type { Page } from '@playwright/test';

// [[WEIR-T-0220]]: the user journeys of COLLIERY-I-0251, end to end against the real e2e
// server (no routing): create a connection in the form, run it and see rows arrive; start and
// stop a resident source; the non-admin view; an admin's Setup in a switched tenant.
//
// The server's state outlives a spec (and a retry), and its seed step ignores errors: each
// journey makes the names, tenants, keys and catalog entries it needs. A create on this server
// is slow (it loads each side's wasm guest, synchronously, in a debug build), so the journeys
// wait on the real responses and poll the API, never on fixed sleeps or the 10s toast. A
// sqlite-lock answer is retried (see `api` / `act` in fixtures); any other refusal fails.

const uniq = (prefix: string) => `${prefix}-${Date.now().toString(36)}`;
const names = async (res: { json(): Promise<unknown> }) => ((await res.json()) as { name: string }[]).map((c) => c.name);

/** Fill the Setup form: frankfurter `latest` → the arrow sink. `tenant` scopes the discover. */
async function fillPair(page: Page, name: string, tenant: string | null) {
  await fillStable(page.getByPlaceholder('my-sync'), name);
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
  else await fillStable(stream, 'latest');
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
        const r = await api(page.request, 'GET', `/runs/${id}`);
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

  await api(page.request, 'DELETE', `/connections/${name}`);
});

test('journey: a resident source starts and stops from its card', async ({ page }) => {
  test.setTimeout(180_000);
  const name = uniq('fx-resident');
  const dest = await ensurePair(page.request);
  const created = await api(page.request, 'POST', '/connections', {
    data: {
      name,
      source: 'frankfurter',
      dest,
      stream: 'latest',
      source_config: {},
      dest_config: {},
      every_secs: 30,
      execution_mode: 'resident',
    },
  });
  await expectOk(created, `create ${name}`);
  const active = async () => {
    const r = await api(page.request, 'GET', `/connections/${name}/runs`);
    await expectOk(r);
    return ((await r.json()) as { state: string }[]).filter((u) => u.state === 'pending' || u.state === 'leased')
      .length;
  };

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

  await api(page.request, 'DELETE', `/connections/${name}`);
});

test('journey: a non-admin key gets a tenant chip, no switcher, no Platform', async ({ page }) => {
  test.setTimeout(90_000);
  const tenant = uniq('na');
  await expectOk(await api(page.request, 'POST', '/tenants', { data: { id: tenant } }), `create tenant ${tenant}`);
  const minted = await api(page.request, 'POST', `/tenants/${tenant}/keys`, {
    data: { name: 'e2e-writer', role: 'write' },
  });
  await expectOk(minted, 'mint key');
  const key: string = (await minted.json()).key;
  expect(key).toMatch(/^weirk_/);

  // Sign in as the non-admin (this init script runs after the fixture's admin one). The key
  // is new, so the server checks it against the store: a sqlite lock there answers 503 (not
  // 401), and the page is loaded again.
  await page.addInitScript((k) => localStorage.setItem('weir_api_key', k), key);
  await expect(async () => {
    const me = nextResponse(page, 'GET', '/auth/me', 15_000);
    await page.goto('/');
    const res = await me;
    await expectOk(res);
    expect(await res.json()).toMatchObject({ tenant, is_admin: false });
  }).toPass({ timeout: 60_000 });
  await expect(page.getByText('Run feed')).toBeVisible();

  // The tenant shows as a chip, not a switcher; no Platform tab, no tenants admin.
  await expect(page.getByText(`⊙ ${tenant}`, { exact: true })).toBeVisible();
  await expect(page.getByRole('combobox', { name: 'Tenant' })).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Setup' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Platform' })).toHaveCount(0);
  await expect(page.getByTitle('administer tenants')).toHaveCount(0);

  await api(page.request, 'DELETE', `/tenants/${tenant}`);
});

test('journey: an admin switched to another tenant onboards and creates there', async ({ page }) => {
  test.setTimeout(180_000);
  const tenant = uniq('tb');
  const name = uniq('fx-tb');
  await expectOk(await api(page.request, 'POST', '/tenants', { data: { id: tenant } }), `create tenant ${tenant}`);

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
  const inB = await api(page.request, 'GET', `/tenants/${tenant}/connections`);
  await expectOk(inB);
  expect(await names(inB)).toContain(name);
  const own = await api(page.request, 'GET', '/connections');
  await expectOk(own);
  expect(await names(own)).not.toContain(name);
  // And B's Operations lists it.
  await page.getByRole('button', { name: 'Operations' }).click();
  await expect(card(page, name)).toBeVisible({ timeout: 30_000 });

  await api(page.request, 'DELETE', `/tenants/${tenant}`);
});
