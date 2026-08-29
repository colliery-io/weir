# First-hour triage

The failures people actually hit in their first hour with weir, and the fix for each.

## "I can't sign in" (fresh install)

The API and UI are authenticated by default. On a **fresh store**, the first
`weir api` / `weir serve` mints the admin key and prints it **once**:

```
fresh store — admin API key minted; save this, it is not shown again:
  weirk_…
```

- Missed it in a terminal? It's only printed at the mint — restarts never re-print.
  Mint another admin key from the box that has the store:
  `weir --db <store> auth token create --name rescue --admin`.
- Docker demo: `docker compose --profile demo logs weir | grep -A1 "admin API key"`.
- Pre-mint deliberately with `weir init` before first serve (scripted setups).

## "unknown source connector …" when creating a connection

The connector isn't staged or cataloged (the error names the package and search path).

- `GET /catalog/available` lists everything onboardable; `POST /catalog/import` (or the
  UI's Setup picker) onboards one.
- For compiled connectors: `WEIR_CONNECTORS_DIR` must point at a staged directory —
  `bash scripts/stage-connectors.sh ./connectors && export WEIR_CONNECTORS_DIR=$PWD/connectors`.
  An **empty or unset** `WEIR_CONNECTORS_DIR` is the classic cause: catalog listings come up
  empty and every compiled-connector reference 404s.

## "… config … missing required field(s)"

The connector declares required config in its schema; the error lists the missing keys.
`GET /connectors/<name>/spec` shows the full config contract (the UI renders the same
schema as a form).

## Runs fail / the dashboard shows red

- The run feed row carries the error; click the connection card for logs and dead-letters.
- Transient failures (rate limits, network blips) retry automatically (3 attempts by
  default — see [Installation → Runtime knobs](../reference/installation.md#runtime-knobs)).
  A run that still fails after retries keeps its final error in the feed.
- A red **"control plane error"** banner across the top means the UI can't reach the
  server (or it's erroring) — the dashboards keep showing last-known data until it clears.

## Port conflicts (Docker demo)

Everything is overridable: `WEIR_HTTP_HOST_PORT` (weir, default 8080),
`WEIR_PG_HOST_PORT` (5432), `WEIR_MSSQL_HOST_PORT` (1433), `WEIR_MINIO_HOST_PORT` (9000).

```bash
WEIR_HTTP_HOST_PORT=18080 WEIR_PG_HOST_PORT=25432 docker compose --profile demo up --build
```

## A schedule keeps firing the old config

It shouldn't: editing a connection re-registers its schedule (config included) on the
next reconcile (each serve poll). If a schedule misbehaves, re-POST the connection —
creation is an upsert by name — and check the run feed for what actually executed.

## Where things live

| What | Where |
| --- | --- |
| Run history + errors | `GET /runs`, `GET /connections/<n>/runs` — or the UI run feed |
| Connector logs | `GET /connections/<n>/logs` — or the run-detail modal |
| Dead letters (rejected records + reasons) | `GET /connections/<n>/dead-letters` |
| Committed cursor / resume point | `GET /connections/<n>/state` |
| Health rollup | `GET /overview` (per connection), `GET /platform/health` (admin) |
