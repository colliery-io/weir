---
id: client-demo-parity-ga4-sheets
level: initiative
title: "End-to-end pipeline demo set — GA4/Sheets/MSSQL/Snowflake/HubSpot/Stripe"
short_code: "WEIR-I-0041"
created_at: 2026-07-15T02:06:55.291441+00:00
updated_at: 2026-07-15T02:12:50.555394+00:00
parent: WEIR-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/active"


exit_criteria_met: false
estimated_complexity: L
initiative_id: client-demo-parity-ga4-sheets
---

# End-to-end pipeline demo set — GA4/Sheets/MSSQL/Snowflake/HubSpot/Stripe Initiative

> **Demo build (2026-07-14).** A representative set of six end-to-end pipelines for an initial demo to
> interested parties — live-tested against real services (no demo without a passing live test). Account/infra
> provisioning is the human's side and is specified in [[WEIR-S-0018]]; this initiative is the build side.

## Context **[REQUIRED]**

The demo estate — a representative pipeline set:

| # | Pipeline | Source in weir today | Dest in weir today |
|---|----------|----------------------|---------------------|
| 1 | HubSpot → Snowflake | ✅ `manifests/hubspot.yaml` | ❌ Snowflake dest |
| 2 | Stripe → Snowflake | ✅ `manifests/stripe.yaml` | ❌ |
| 3 | Google Sheets → Snowflake | ❌ | ❌ |
| 4 | Google Analytics (GA4) → Snowflake | ❌ | ❌ |
| 5 | MSSQL → Snowflake | ❌ | ❌ |
| 6 | Snowflake → HubSpot (rETL) | ❌ Snowflake source | ✅ `dest-manifests/hubspot-dest.yaml` |

Load-bearing gap: **Snowflake** (both sides — appears in all six pipelines). Second tier: **MSSQL** (compiled
connector), **GA4** (needs POST-with-body in the declarative runtime + Google service-account auth), **Sheets**
(needs the same Google auth). HubSpot + Stripe sources and the HubSpot rETL dest exist and only need live-test
credentials ([[WEIR-I-0014]] SOPS flow, [[WEIR-T-0067]]).

## Goals & Non-Goals **[REQUIRED]**

**Goals:**
- All six pipelines run on weir end-to-end against real services, proven by the live suite
  (`angreal test connectors-live`) and a repeatable demo setup script.
- POST-with-body request support in the shared declarative `rest` runtime (also closes the known
  Notion body-cursor gap in `manifests/README.md`).
- Host-side Google service-account (JWT-grant) auth per [[WEIR-A-0033]]/[[WEIR-A-0037]] — secrets never enter
  the guest.
- Snowflake destination + source (SQL API, key-pair JWT auth).
- MSSQL source connector (compiled, like `postgres`).

**Non-Goals:**
- MSSQL CDC/change-tracking parity — full refresh + cursor incremental is the demo bar; CDC is follow-up.
- GA4 backfill/attribution depth — a representative report stream set, not the full GA4 surface.
- Snowflake bulk-load performance (PUT/COPY staging) — SQL API INSERT batching is acceptable at demo scale.

## Detailed Design **[REQUIRED]**

Dependency spine:

1. **POST-with-body** in `rest` runtime (manifest field + engine + importer) — blocks GA4 and the Snowflake
   SQL API approach.
2. **Google SA auth** host-side (JWT assertion → token endpoint → bearer injection, cached + refreshed) —
   blocks GA4 + Sheets.
3. **Snowflake key-pair JWT auth** host-side (same injection seam, RS256 self-signed JWT) — blocks Snowflake
   source + dest.
4. Connector work on top: `snowflake` dest + source manifests (SQL API `/api/v2/statements`), `ga4.yaml`,
   `google-sheets.yaml`, and the compiled `mssql` source (tiberius; integration compose service like Postgres).
5. **Demo setup script** that provisions all six connections against a running weir (catalog import → onboard →
   connections → modes), standing the estate up from the spec in one command.

Design decisions to ratify during design phase: whether Snowflake rides `rest`/`rest-dest` (SQL API) or gets a
thin compiled connector; where the auth-scheme vocabulary for the two new host-side schemes lands (touches
[[WEIR-I-0033]]/[[WEIR-I-0034]] string-key coupling).

## Alternatives Considered **[REQUIRED]**

- **Postgres stand-in warehouse for the demo.** Rejected by the human — the demo must be believable against a real warehouse + services stack, live-tested.
- **Fixture look-alike sources for GA4/Sheets/MSSQL.** Rejected — not real data; fails the "no demo without a
  passing live test" bar.
- **Compiled Snowflake connector via a driver crate first.** Deferred decision — SQL API on the declarative
  runtime is the smaller step and exercises runtime features we need anyway; revisit if INSERT batching or type
  fidelity falls short.

## Implementation Plan **[REQUIRED]**

Decomposed tasks (dependency order; MSSQL parallel-safe from the start):

- [ ] [[WEIR-T-0154]] POST-with-body requests in the declarative rest runtime
- [ ] [[WEIR-T-0155]] Host-side Google service-account JWT auth scheme
- [ ] [[WEIR-T-0156]] Host-side Snowflake key-pair JWT auth scheme
- [ ] [[WEIR-T-0157]] Snowflake destination (SQL API)
- [ ] [[WEIR-T-0158]] Snowflake source (SQL API) — rETL feed
- [ ] [[WEIR-T-0159]] GA4 source manifest + live test
- [ ] [[WEIR-T-0160]] Google Sheets source manifest + live test
- [ ] [[WEIR-T-0161]] MSSQL source connector (compiled) + integration compose
- [ ] [[WEIR-T-0162]] Demo setup script + secret bundles + six-pipeline live validation

**Exit criteria:** all six pipelines green in the live suite against provisioned accounts ([[WEIR-S-0018]]);
demo script stands the estate up from a clean weir in one command.
