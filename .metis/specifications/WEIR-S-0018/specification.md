---
id: demo-account-provisioning-accounts
level: specification
title: "Demo account provisioning — accounts/infra for the live pipeline demo"
short_code: "WEIR-S-0018"
created_at: 2026-07-15T02:06:56.974022+00:00
updated_at: 2026-07-15T02:06:56.974022+00:00
parent: WEIR-V-0001
blocked_by: []
archived: false

tags:
  - "#specification"
  - "#phase/discovery"


exit_criteria_met: false
initiative_id: NULL
---

# Demo account provisioning — accounts/infra for the live pipeline demo

## Overview **[REQUIRED]**

The human-side checklist for [[WEIR-I-0041]] (the pipeline demo). Each account below yields one SOPS-encrypted
secret bundle (`angreal secrets edit <slug>` → `secrets/<slug>.enc.json`, [[WEIR-I-0014]] flow) that the live
suite (`angreal test connectors-live`) and the demo setup script consume. Nothing here blocks the runtime/auth
build work — but **GA4 must be set up first** (24–48h data lag on a fresh property).

## Requirements **[REQUIRED]**

### 1. Google Cloud project — covers GA4 + Sheets · slugs `google-analytics` + `google-sheets` · ⚠️ do first

> One GCP project + one SA key, landed in **two** bundles (the live harness maps
> `secrets/<slug>.json` → `manifests/<slug>.yaml` 1:1, [[WEIR-T-0159]]). Each bundle embeds the same
> `service_account_key` plus its connector's fields — shape: `secrets/google-analytics.example.json`
> (`property_id`, `start_date`) / the sheets example ([[WEIR-T-0160]]).

- [ ] Create one GCP project; enable **Google Analytics Data API** + **Google Sheets API**.
- [ ] Create a **service account**; download the **JSON key** (one key drives both connectors).
- [ ] **GA4:** a property with real traffic (a site you control — the Google demo property can't grant SA
      access). Admin → Property Access Management → add the SA email as **Viewer**. Fresh properties take
      **24–48h** to show report data.
- [ ] **Sheets:** create a test spreadsheet (a few tabs of tabular data); share with the SA email (Viewer).
- **Bundle:** service-account JSON key, GA4 property ID (numeric), spreadsheet ID (from the URL).

### 2. Snowflake · slug `snowflake`

- [ ] Trial account is fine (30 days, instant).
- [ ] **Key-pair auth, not password:** generate an RSA keypair; `ALTER USER weir_demo SET RSA_PUBLIC_KEY='...'`.
- [ ] Role with an XS warehouse + `CREATE/INSERT/SELECT` on a demo database (dest for pipelines 1–5, source
      for the rETL pipeline).
- **Bundle:** account identifier (`org-account`), username, private key PEM, role, warehouse, database, schema.

### 3. HubSpot · slug `hubspot`

- [ ] Free CRM account. **Private App** with `crm.objects.contacts` **read + write** scopes (read = source,
      write = rETL dest; `dest-manifests/hubspot-dest.yaml` upserts contacts by email).
- [ ] Seed a handful of contacts so the source has data.
- **Bundle:** private-app token, portal ID.

### 4. Stripe · slug `stripe`

- [ ] Any account, **test mode**. **Restricted key**: read on customers/charges/invoices.
- [ ] Seed test data (dashboard or `stripe fixtures`). Shape: `secrets/stripe.example.json`.
- **Bundle:** restricted test-mode key.

### 5. MSSQL — nothing to provision

Runs as `mcr.microsoft.com/mssql/server:2022` in the integration compose stack, seeded like the Postgres
integration resources ([[WEIR-T-0161]]). **Only flag:** if the target SQL Server is old (2016-era) or leans on
CDC-by-agent specifics, say so and the version/feature set gets matched.

### Non-Functional Requirements

| ID | Requirement | Rationale |
|----|-------------|-----------|
| NFR-1 | All credentials land as SOPS bundles; none in plaintext, env files, or manifests | [[WEIR-A-0033]]/[[WEIR-A-0037]] — secrets are consumed, not managed; never enter the guest |
| NFR-2 | GA4 provisioned ≥48h before the first live-test run | Fresh-property report-data lag |
| NFR-3 | All accounts are demo/trial-tier, isolated from any production tenant | Demo hygiene |

## Constraints **[CONDITIONAL: Has Constraints]**

### Technical Constraints
- Snowflake access must be key-pair JWT (the auth scheme [[WEIR-T-0156]] builds) — password auth won't be wired.
- The Google SA key is a single credential shared by two connectors (GA4 + Sheets); one bundle, one rotation.
