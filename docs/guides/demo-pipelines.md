# Demo pipelines

Stand a representative ingestion estate up on a clean weir in one command — a showcase
of weir's connectors working together end-to-end against real services, not a slide.

The estate ([`demo/pipelines.toml`](https://github.com/colliery-io/weir/blob/main/demo/pipelines.toml))
is **six pipelines**: HubSpot, Stripe, Google Sheets, Google Analytics (GA4), and MSSQL
each landing in **Snowflake**, plus a reverse-ETL flow from **Snowflake back to HubSpot**.
Snowflake is written upsert-by-key, so re-syncs are idempotent.

The estate is *data*. Point it at a different set of pipelines by editing that one TOML
file — the connectors to onboard and the connections to create, with which secret-bundle
field feeds which config key. No code changes.

## What you provide

Each pipeline's credentials live in a SOPS-encrypted bundle under `secrets/<slug>.enc.json`
(see [`the provisioning spec`](https://github.com/colliery-io/weir/blob/main/.metis/specifications/WEIR-S-0018/specification.md)
and the `secrets/<slug>.example.json` templates). Create them with:

```bash
angreal secrets edit snowflake      # account, user, private_key, database, schema, warehouse
angreal secrets edit hubspot        # api_key (private-app token, contacts read+write)
angreal secrets edit stripe         # api_key (restricted, test mode)
angreal secrets edit google-analytics   # service_account_key, property_id, start_date
angreal secrets edit google-sheets      # service_account_key, spreadsheet_id, tab
```

MSSQL needs no bundle — its integration container supplies the credentials.

> **Set up Google Analytics first.** A fresh GA4 property takes 24–48h to show report
> data, so provision it before anything else; its live run is the last to go green.

## Bring-up

```bash
# MSSQL source pipeline uses the integration container.
angreal integration up

# Onboard the seven connectors and create the six connections against a clean weir.
angreal demo pipelines
```

The task decrypts whatever bundles are present, builds + stages the connectors, starts the
control plane, and drives its HTTP API to onboard each connector and create each connection —
**idempotent**, so re-running reconciles rather than duplicating. It prints a per-pipeline
summary:

```
▸ creating connections
  ✓ hubspot-to-snowflake: provisioned
  ✓ stripe-to-snowflake: provisioned
  ✓ sheets-to-snowflake: provisioned
  ✓ ga4-to-snowflake: provisioned
  ✓ mssql-to-snowflake: provisioned
  ✓ snowflake-to-hubspot: provisioned

▸ 6/6 connections provisioned  →  http://localhost:8090
```

A pipeline whose bundle is missing is **still provisioned** — the connection is created —
but flagged `live run needs bundle(s): <slug>`. So the estate stands up today and each
pipeline goes live as its account lands. Secrets are decrypted at run time and injected
**host-side** ([the credential model](../explanation/connector-model.md)); they never enter
the connector sandbox and are never written to the store in plaintext.

## In the room

Open the printed URL (default `http://localhost:8090`). The UI lists the six connections;
for each you can:

- **Run** it and watch the live feed move (records read → written), the row counts, and the
  typed schema weir captured.
- Open **Snowflake** alongside and `SELECT` the target table (`hubspot_contacts`,
  `stripe_customers`, `sheet_rows`, `ga4_traffic`, `mssql_contacts`) to show the rows landing.
- Run **snowflake-to-hubspot** and show the contacts appearing in HubSpot — reverse-ETL out
  of the warehouse, the same platform, no second tool.

## Prove it before you demo

Never demo a pipeline you haven't seen pass. The live suite runs each pipeline end-to-end
against the real accounts and asserts records land (and, for Snowflake, reads the rows back):

```bash
angreal test connectors-live
```

Pipelines whose bundles are absent skip cleanly, so this grows green as accounts come online.
Sequence GA4 last — it can't pass until its data window has elapsed.

## Teardown

```bash
angreal demo down          # stop the control plane, clear the demo state
angreal integration down   # stop the MSSQL container
```
