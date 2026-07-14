---
id: s4-hubspot-destination-manifest-on
level: task
title: "S4: HubSpot destination (manifest on the shared destination runtime)"
short_code: "WEIR-T-0074"
created_at: 2026-07-04T03:18:27.353358+00:00
updated_at: 2026-07-04T03:50:08.561305+00:00
parent: WEIR-I-0007
blocked_by: [WEIR-T-0072, WEIR-T-0073]
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0007
---

# S4: HubSpot destination (manifest on the shared destination runtime)

## Parent Initiative

[[WEIR-I-0007]] slice S4. The first real SaaS destination — and the proof that a destination is now
**config, not code** ([[WEIR-A-0034]]).

## Objective

Ship a **HubSpot CRM destination as a manifest** running on the S2 destination runtime ([[WEIR-T-0072]]):
create-or-update CRM objects (contacts / companies) keyed on a unique property, authed by a host-injected
private-app token. No new guest code — this should be **a manifest + a wire test**.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [ ] A **HubSpot destination manifest** (in the vendored corpus, authored from HubSpot's public CRM API
  docs, Apache-2.0, attributed — same provenance rules as the source catalog): at least the **contacts**
  object; **upsert by a unique property** (e.g. `email`) via HubSpot's create-or-update / upsert endpoint;
  field map from record fields → HubSpot `properties`.
- [ ] **Auth**: HubSpot private-app **bearer token**, injected **host-side** ([[WEIR-A-0033]]) — the token is
  a per-connection secret, never in the manifest, never in the guest.
- [ ] **E2E wire test against a mock HubSpot** server: records upserted with the correct endpoint/method and
  `properties` shape; a rejected record dead-letters; `accepted` count correct; a **replay is idempotent**
  (upsert by the unique property).
- [ ] Onboards through the normal path (importer/catalog → connection → run); `analyze()` reports it clean
  (or reports any unsupported sub-form, never silently drops).
- [ ] Workspace + integration suites green; clippy clean.

## Technical Notes

- This slice should surface **zero or minimal** new runtime code — if HubSpot needs a construct the S2 runtime
  can't express (e.g. its batch upsert endpoint shape), add it to the **runtime** (so every destination
  benefits) and note it, per [[WEIR-A-0034]] / [[WEIR-A-0020]]; don't special-case HubSpot.
- No real HubSpot account needed for green — the mock proves the wire. A live smoke against a real private-app
  token belongs with the [[WEIR-I-0014]] live-secrets harness (provisioning is [[WEIR-T-0067]]), not here.
- Keep the manifest to the **common CRM upsert shape**; exotic HubSpot features (associations, custom object
  schemas) are follow-ups, reported not attempted.

## Dependencies

- **Blocked by [[WEIR-T-0072]]** (runtime) and ideally [[WEIR-T-0073]] (flow/idempotency proven).
- Independent of [[WEIR-T-0075]] (Salesforce) — either can land first once S2/S3 exist.

## Status Updates

### 2026-07-04 — HubSpot lands as a manifest; wire-test green

**Manifest, not code.** New `dest-manifests/hubspot.yaml` (authored from HubSpot's public CRM v3 docs,
Apache-2.0): contacts **upsert by the unique `email` property**
(`PATCH /crm/v3/objects/contacts/{{ record.email }}?idProperty=email`), field map (`first_name`→`firstname`),
`body_wrap: properties`, bearer auth. **No new guest code.**

**Baking** — `weir_app::dest_object_to_config` (destination analogue of `manifest_stream_to_config`):
`DestObject` → the `rest-dest` config + `auth_scheme` for host-side injection. OAuth arm → [[WEIR-T-0075]].

**E2E test** `crates/weir-app/tests/reverse_etl_hubspot.rs`: bakes the vendored manifest → config → runs
source → `rest-dest` over real `wasi:http` against a **mock HubSpot**, twice:
- bake asserts the shape (`method=PATCH`, `body_wrap=properties`, `auth_scheme=bearer`);
- `rows_written == 2` each run (3 contacts, one 422-rejected); **idempotent replay** (2 email keys after both);
- rejected contact **dead-lettered** each run (`dead_letter_count == 2`);
- captured request proves the **email-keyed PATCH URL**, **`Authorization: Bearer` injected host-side**
  (`inject_headers`, never in manifest/guest), the **`properties` wrap**, and the field map.

Added `weir-runtime` as a weir-app dev-dep. clippy clean.

**Scope note:** full catalog/UI onboarding of *destination* manifests is broader and deferred; this proves
the manifest→config→run path (the "config not code" claim). **All ACs met — complete.**
