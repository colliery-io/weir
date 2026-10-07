import { test, expect, api, expectOk } from './fixtures';

// [[WEIR-T-0080]]: clicking a connection card opens the Aurora Modal connection detail
// (requires a seeded `fx-demo` connection on the running server).
test('operations: card opens the connection-detail modal', async ({ page }) => {
  await page.goto('/');
  const card = page.getByTestId('connection-card').filter({ hasText: 'fx-demo' });
  await expect(card).toBeVisible();

  await card.first().click();
  await expect(page.getByRole('dialog', { name: 'Connection detail' })).toBeVisible();
  // Lineage panel ([[WEIR-T-0101]]): the source→dest chain renders.
  const dialog = page.getByRole('dialog', { name: 'Connection detail' });
  await expect(dialog.getByRole('heading', { name: 'Lineage' })).toBeVisible();
  await expect(dialog.getByRole('heading', { name: 'Dead-letters' })).toBeVisible();
  await expect(dialog.getByRole('heading', { name: 'Logs' })).toBeVisible();
});

// [[WEIR-T-0219]]: a run-feed row opens that run's detail (GET /runs/{id}), and the feed
// pages back through history (GET /runs?before=…). The seeded `fx-demo` run is enough.
test('operations: a run-feed row opens the run detail; the feed pages', async ({ page }) => {
  // Queue a run of its own so the feed is sure to list one for fx-demo.
  // `api` retries a sqlite-lock answer of the e2e server (see fixtures).
  const queued = await api(page.request, 'POST', '/connections/fx-demo/run');
  await expectOk(queued);
  const feed = await api(page.request, 'GET', '/runs');
  const feedBody = await feed.text();
  expect(feedBody, `GET /runs → ${feed.status()}`).toContain('fx-demo');

  // [[WEIR-T-0224]]: every feed row carries started_at / finished_at — RFC 3339 UTC or null.
  const rfc3339 = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z$/;
  const rows = JSON.parse(feedBody) as Array<Record<string, unknown>>;
  for (const r of rows) {
    for (const k of ['started_at', 'finished_at']) {
      expect(k in r, `run ${r.id} has ${k}`).toBeTruthy();
      const v = r[k];
      if (v !== null) expect(String(v)).toMatch(rfc3339);
    }
  }

  await page.goto('/');
  await expect(page.getByRole('columnheader', { name: 'started', exact: true })).toBeVisible();
  const row = page.getByRole('row', { name: /^run \d+ · fx-demo$/ }).first();
  await expect(row, `feed rows in the page; GET /runs gave: ${feedBody.slice(0, 400)}`).toBeVisible();
  // The start column: "… ago" / "just now" with the UTC time as the tooltip, or "—" until it starts.
  const started = row.getByTestId('run-started');
  await expect(started).toHaveText(/^(—|just now|in \d+[a-z]+|\d+[a-z]+ ago)$/);
  if ((await started.textContent()) !== '—') {
    await expect(started).toHaveAttribute('title', /^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2} UTC$/);
  }
  await row.click();

  const dialog = page.getByRole('dialog', { name: /^Run #\d+$/ });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByText('fx-demo', { exact: true }).first()).toBeVisible();
  await expect(dialog.getByText('started', { exact: true })).toBeVisible();
  await expect(dialog.getByText('duration', { exact: true })).toBeVisible();
  await expect(dialog.getByRole('heading', { name: 'Committed state · connection' })).toBeVisible();
  await expect(dialog.getByText('committed cursor', { exact: true })).toBeVisible();
  await expect(dialog.getByRole('heading', { name: 'Dead-letters · connection' })).toBeVisible();
  await expect(dialog.getByRole('heading', { name: 'Logs · connection' })).toBeVisible();

  // From the run to its connection.
  await dialog.getByRole('button', { name: 'Open connection' }).click();
  await expect(page.getByRole('dialog', { name: 'Connection detail' })).toBeVisible();
  await page.keyboard.press('Escape');

  // One seeded run: the first older page is empty, so the feed says so.
  await page.getByRole('button', { name: 'Load older runs' }).click();
  await expect(page.getByText('No older runs.')).toBeVisible();
});
