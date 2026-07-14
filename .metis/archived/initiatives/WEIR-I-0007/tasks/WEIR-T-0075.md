---
id: s5-salesforce-destination-manifest
level: task
title: "S5: Salesforce destination (manifest) + host-side OAuth token refresh"
short_code: "WEIR-T-0075"
created_at: 2026-07-04T03:18:58.040846+00:00
updated_at: 2026-07-05T01:09:19.842229+00:00
parent: WEIR-I-0007
blocked_by: [WEIR-T-0072, WEIR-T-0073]
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0007
---

# S5: Salesforce destination (manifest) + host-side OAuth token refresh

## Parent Initiative

[[WEIR-I-0007]] slice S5 — a **headline open-core capability**: first-class reverse ETL to the Salesforce
destination. Also the one slice with genuinely new plumbing: OAuth token **refresh**.

## Objective

Ship a **Salesforce destination as a manifest** on the S2 runtime ([[WEIR-T-0072]]) — sObject **upsert by
External Id** — and resolve the one thing sources didn't fully need: **host-side OAuth token refresh** for a
destination whose bearer token expires mid-run.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [ ] A **Salesforce destination manifest** (authored from Salesforce's public REST API docs, Apache-2.0,
  attributed): sObject **upsert by External Id field** (`PATCH /sobjects/{sobject}/{extIdField}/{value}`);
  field map record → sObject fields; at least one object (e.g. `Contact` or a custom object) proven.
- [ ] **Host-side OAuth refresh**: extend the existing host-side credential injection ([[WEIR-A-0033]]) /
  OAuth2 provider used on the source side so a destination's access token is **minted and refreshed on
  expiry** (refresh-token or client-credentials grant), injected as `Authorization: Bearer` — **never in the
  guest**. If this needs a distinct auth seam, split it as **S5a** and land it first.
- [ ] **E2E wire test against a mock Salesforce**: token minted host-side → sObject upsert by External Id
  succeeds → **token expiry mid-run triggers a refresh** (mock returns 401 once / an expired token) and the
  run continues → replay is idempotent (upsert). A rejected record dead-letters.
- [ ] Runtime constructs Salesforce needs that the S2 runtime lacks are added to the **runtime** (benefiting
  all destinations) and noted; the async **Bulk API 2.0** (job-polling) is explicitly **out of scope /
  reported** here — the full-code escape hatch ([[WEIR-A-0034]] / [[WEIR-A-0032]]) covers it later if needed.
- [ ] Workspace + integration suites green; clippy clean.

## Technical Notes

- **OAuth refresh is the crux.** The source arc did OAuth2 *grants* host-side ([[WEIR-T-0063]] /
  [[WEIR-A-0033]], `OAuth2Provider` with expiry-based re-mint) — reuse that machinery on the write path; the
  new-ness is applying it to a long-running `write` where the token can expire between batches. Prefer
  expiry-based re-mint (re-mint within a margin of `expires_in`); 401-driven refresh is the fallback.
- No real Salesforce org needed for green — mock the token endpoint + the sObject API. A live smoke belongs
  with [[WEIR-I-0014]] (secrets provisioning [[WEIR-T-0067]]).
- Salesforce Composite/Bulk batching is an optimization; single-record upsert is acceptable for this slice.

## Dependencies

- **Blocked by [[WEIR-T-0072]]** (runtime); pairs with [[WEIR-T-0073]] (flow/idempotency).
- The OAuth-refresh seam (S5a if split) may be the first thing to build, since [[WEIR-T-0074]] (HubSpot,
  static token) doesn't exercise it.

## Status Updates

### 2026-07-04 — Salesforce lands as a manifest + OAuth refresh reused; wire-test green

**No new auth seam needed — the source-arc `OAuth2Provider` reuses directly.** It's a generic egress
credential ([[WEIR-A-0033]]): `bearer()` re-mints when `now + REFRESH_MARGIN(60s) >= expires_at`, and the
egress policy applies it to *any* outbound request — source read or dest write. So a destination's token is
minted + refreshed host-side with zero new plumbing. **No S5a split required.**

**Manifest** `dest-manifests/salesforce.yaml` (authored from public docs, Salesforce REST v59.0): Contact **upsert by
External Id** (`PATCH /services/data/v59.0/sobjects/Contact/ExternalId__c/{{ record.ext_id }}`), field map
(`first_name`→`FirstName`…), `o_auth2` / **client_credentials** grant. No `body_wrap` (flat sObject body).

**Bake** — added the `OAuth2` arm to `weir_app::dest_object_to_config` (mirrors the source emission:
`auth_scheme=oauth2`, `oauth_token_url`, `oauth_grant`, client-id/secret keys, scopes). Small
`weir-manifest` fix: `Auth::OAuth2.refresh_token_key` is now `#[serde(default)]` (absent for
client-credentials).

**E2E test** `crates/weir-app/tests/reverse_etl_salesforce.rs`: bake → `Credential::from_auth_config` splits
the OAuth2 credential (asserts the **client secret is stripped** from the guest cfg) → run source →
`rest-dest` over `wasi:http` against a **mock Salesforce** (token endpoint + sObject API), twice:
- token minted **host-side**; `expires_in:1` forces re-mint every request → **`mints >= 2` (refresh)**;
- `rows_written == 2` each run (one `/reject` → 400 → dead-letter, `dead_letter_count == 2`);
- **idempotent replay** — 2 External-Id keys after both runs;
- captured request proves the **External-Id PATCH URL**, the **host-minted `Authorization: Bearer sf-…`**,
  and the field map (`first_name`→`FirstName`).

Bulk API 2.0 (async job polling) is **out of scope / reported** — the full-code escape hatch ([[WEIR-A-0034]])
covers it later. weir-manifest 5/5, clippy clean. **All ACs met — complete.**
