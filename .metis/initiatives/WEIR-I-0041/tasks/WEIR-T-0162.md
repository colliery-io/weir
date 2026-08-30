---
id: demo-setup-script-secret-bundles
level: task
title: "Demo setup script + secret bundles + six-pipeline live validation"
short_code: "WEIR-T-0162"
created_at: 2026-07-15T02:09:18.276619+00:00
updated_at: 2026-08-30T03:09:27.516591+00:00
parent: WEIR-I-0041
blocked_by: [WEIR-T-0157, WEIR-T-0158, WEIR-T-0159, WEIR-T-0160, WEIR-T-0161]
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0041
---

# Demo setup script + secret bundles + six-pipeline live validation

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0041]]

## Objective **[REQUIRED]**

The capstone: one command stands a representative estate up on a clean weir — catalog import, connector
onboarding, six connections (HubSpot→Snowflake, Stripe→Snowflake, Sheets→Snowflake, GA4→Snowflake,
MSSQL→Snowflake, Snowflake→HubSpot rETL) with sync/write modes matching the demo spec — plus the
live suite proving each end-to-end before anyone demos anything.

## Acceptance Criteria **[REQUIRED]**

*(Amended 2026-08-30 at close: the six-green LIVE data run is CANCELLED — Dylan will not provision the
cloud accounts ([[WEIR-S-0018]] archived). AC3 closes on its verified mechanism; the estate's designed
degraded mode — provisioned-but-not-live, per-pipeline "needs bundle" flags — is the shipped state.)*

- [x] An angreal task (`angreal demo pipelines`) provisions all six connections against a running weir
      via the control-plane API; idempotent re-run — verified live 2026-07-16.
- [x] Secret bundles wired: SOPS decrypt → per-side config → host-side injection; MSSQL creds from the
      compose service. (Bundles themselves cancelled; the plumbing is proven and user-consumable.)
- [x] The `--validate` scripted smoke (runs each pipeline, polls `/runs`, reports rows landed) is built and
      its path proven; the six-green data run is cancelled with the accounts.
- [x] Runbook shipped (`docs/guides/demo-pipelines.md` + nav).

## Implementation Notes **[CONDITIONAL: Technical Task]**

### Technical Approach
Compose the existing pieces: `angreal ui demo`'s catalog-seed flow, the weir-soak provisioner's API client
([[WEIR-T-0122]]) for connection creation, and the live-suite SOPS decryption from [[WEIR-T-0066]]. The script
is config-driven (a small TOML/JSON describing the six pipelines) so the next client estate is a data change.

### Dependencies
All build tasks: [[WEIR-T-0157]], [[WEIR-T-0158]], [[WEIR-T-0159]], [[WEIR-T-0160]], [[WEIR-T-0161]] — plus
provisioned accounts ([[WEIR-S-0018]]) and the existing HubSpot/Stripe manifests (credentials via
[[WEIR-T-0067]]'s flow).

### Risk Considerations
GA4's 24–48h data lag is the schedule risk on the final validation — everything else can go green before it;
sequence the GA4 live check last and don't let it gate the other five.

## Status Updates **[REQUIRED]**

### 2026-07-16 — design: data-driven spec + HTTP provisioning; validation self-arms

**Onboarding seam confirmed:** `add_connection` auto-resolves a manifest source / dest-manifest **once it's in
the catalog** (`resolve_manifest_source`/`_dest`), so provisioning = onboard connectors + create connections,
both over the control-plane HTTP API (Bearer admin key). All needed routes exist: `POST /catalog/import` with
`manifest_name` (source manifests: hubspot/stripe/google-analytics/google-sheets), `dest_manifest_name`
(hubspot-dest), or `package` (compiled: snowflake/mssql); `POST /connections` (`ConnectionDto`). No new CLI
surface needed — the task drives the same API the UI does.

**Six pipelines** (Snowflake on the warehouse side except the rETL flow):
1 hubspot→snowflake · 2 stripe→snowflake · 3 google-sheets→snowflake · 4 google-analytics→snowflake ·
5 mssql→snowflake · 6 snowflake→hubspot-dest (rETL). Snowflake writes upsert-by-key.

**Deliverables:**
- `demo/pipelines.toml` — the estate as **data** (connectors to onboard + the six connections w/ modes,
  keys, cursor, and which bundle fields feed which config keys). Next client = a data change.
- `.angreal/task_demo.py`: `demo pipelines` — decrypt the 4 SOPS bundles (reuse the connectors-live
  block), bring up MSSQL compose, stage compiled guests + point at manifests, `weir init` + detached `weir
  api`, then POST catalog-imports + connections from the spec. **Idempotent** (catalog import + add_connection
  both upsert). `demo down` tears down. Prints the UI URL + a per-pipeline "provisioned / live-run needs
  bundle X" summary.
- connectors-live `snowflake_demo_pipelines_live` test (manifest_corpus): provisions all six via the App API,
  runs each, asserts records land + **Snowflake row-count read-back smoke** — self-arming on the bundles
  ([[WEIR-S-0018]]); skips cleanly without them, like the sibling live tests.
- `docs/guides/demo-pipelines.md` runbook: bring-up, teardown, what to click in the UI.

**Reality:** the six-green live run needs the cloud bundles (the human's [[WEIR-S-0018]] step) + GA4's 24–48h
lag, so it self-arms exactly like T-0157–0160's live tests. What I verify now without creds: the task stands up
weir, onboards all six connectors, and creates all six connections idempotently (secrets are just config
values); MSSQL is real-runnable via its container.

### 2026-07-16 — built + VERIFIED LIVE (provisioning); validate smoke + runbook done

**Ran `angreal demo pipelines` against a clean weir — all six connections provisioned, verified live:**
```
ga4-to-snowflake      rest -> snowflake     stream=traffic
hubspot-to-snowflake  rest -> snowflake     stream=contacts
mssql-to-snowflake    mssql -> snowflake    stream=dbo.contacts
sheets-to-snowflake   rest -> snowflake     stream=rows
snowflake-to-hubspot  snowflake -> rest-dest stream=contacts   (rETL)
stripe-to-snowflake   rest -> snowflake     stream=customers
```
The onboarding seam resolved exactly as designed: manifest sources → `rest` (config baked), dest-manifest →
`rest-dest`, compiled `snowflake`/`mssql` pass through by name. **Idempotency confirmed:** a second run kept
exactly 6 connections, no dupes. The catalog shows all 7 connectors onboarded (verified via `GET /catalog`).

**Deliverables:**
- `demo/pipelines.toml` — estate-as-data (7 connectors, 6 connections; `auth_scheme:
  snowflake_keypair_jwt` added to the compiled-Snowflake configs since only manifests emit it automatically).
- `.angreal/task_demo.py` — `demo pipelines` (SOPS-decrypt present bundles → stage → `weir init` + mint
  admin token + detached `api` → POST catalog-imports + connections from the spec; idempotent; per-pipeline
  summary flagging missing bundles) + `--validate` (runs each pipeline once, polls `/runs`, reports rows
  landed — the AC3 scripted smoke; for a Snowflake-dest pipeline `rows_written` IS the warehouse-accepted
  count) + `demo down`. Validate's `/run`+`/runs` path proven live (enqueue → pending → run row).
- `docs/guides/demo-pipelines.md` runbook (bring-up, secrets, what to click, prove-before-you-demo,
  teardown) + mkdocs nav.

**AC status:** AC1 ✓ (provisions all six via the control-plane API, idempotent — verified live). AC2 ✓ (SOPS
decrypt → per-side config → host-side injection via orchestrator `from_auth_config`; MSSQL from compose). AC3
✓ mechanism (`--validate` scripted smoke reports Snowflake rows landed; the six-green **data** run self-arms on
the [[WEIR-S-0018]] bundles + GA4 window, GA4 sequenced last — like the sibling live tests). AC4 ✓ (runbook).
The only thing gated on the human is the live data run once the cloud accounts exist. Awaiting review.

### 2026-08-30 — closed; live data run cancelled by decision

Dylan will not provision the cloud accounts ([[WEIR-T-0067]] and [[WEIR-S-0018]] archived). Everything
buildable was built and verified live in July (provisioning, idempotency, `--validate` smoke path,
runbook); the estate ships in its designed degraded mode — provisioned-but-not-live with per-pipeline
"needs bundle" flags — and anyone with their own accounts can light it up via `angreal secrets edit`.
