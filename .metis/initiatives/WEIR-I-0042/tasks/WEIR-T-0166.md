---
id: validate-connector-existence
level: task
title: "Validate connector existence + config shape at POST /connections (4xx, not 201-then-fail)"
short_code: "WEIR-T-0166"
created_at: 2026-08-16T15:24:03.074650+00:00
updated_at: 2026-08-25T02:42:39.355412+00:00
parent: WEIR-I-0042
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0042
---

# Validate connector existence + config shape at POST /connections (4xx, not 201-then-fail)

## Parent Initiative

[[WEIR-I-0042]]

## Objective **[REQUIRED]**

POST /connections accepts any source/dest name — `connector_ref` fabricates `weir-<name>-pkg` without checking existence — so a typo returns 201 and surfaces later as a failed run; config-shape errors behave the same way. Validate connector existence and config shape at creation time and return 4xx with a clear reason.

## Evidence (2026-08-16 alpha review)

- `crates/weir-app/src/lib.rs:103-112` — `connector_ref` fabricates the package ref with no existence check (verified).
- Both review assessors independently ranked 201-then-fail among the top "mysterious failure" sources for a first-time user.
- Related same-class footgun, close if cheap: unknown `{{ config['key'] }}` template keys render as empty strings at runtime — `crates/connectors/rest/src/lib.rs:240` — a creation-time warning for unresolvable keys referenced by the manifest would catch the same typo class.

## Acceptance Criteria **[REQUIRED]**

- [x] POST /connections (and the tenant-scoped variant) with an unknown source or dest → **404** naming the connector, the missing package, the search path, and the catalog endpoints
- [x] Config missing requireds per the connector's config_schema → **400** listing the missing field(s) and pointing at GET /connectors/{name}/spec *(400 not 422 — reuses the existing `AppError::Config → BAD_REQUEST` mapping)*
- [x] Upsert-by-name semantics for valid payloads unchanged; `crates/weir-api/tests/api.rs` covers both rejection paths (`create_rejects_unknown_connector_and_missing_required_config`) and asserts nothing persists
- [x] Error bodies are human-useful — plain-language JSON `{"error": …}` bodies ready for [[WEIR-T-0167]]'s toasts

## Implementation Notes

The existence check must consult the same resolution order the executor uses (tenant-scoped artifact dir + shared connectors dir + catalog registrations — see `scope_wasm_to_tenant` and the ingress staging paths) or it will reject connectors that would in fact run. Mode/cadence validation already exists in weir-app (`validate_connection_modes` area, ~L1844) — this extends the same creation-time gate, not a new mechanism.

## Status Updates **[REQUIRED]**

**2026-08-25 — implemented + tested (ralph run).**

- `crates/weir-app/src/lib.rs` `add_connection`: after manifest resolution, two new gates — `App::validate_connector_resolves` (mirrors the executor's search order: `<dir>/<tenant>/<pkg>` then `<dir>/<pkg>`, keyed on `package.toml` presence; misses → `AppError::NotFound` with the connector name, package, path, and catalog endpoints) and `validate_required_config` (parses the side's config JSON, loads the resolved ref's spec — HANDLE-cached — and demands every key in the JSON-Schema `required` list is present and non-null; failures → `AppError::Config` listing the fields). Both run on the RESOLVED refs/configs, so manifest-baked sources validate against the merged config on the `rest` runtime.
- Tests: new `api.rs::create_rejects_unknown_connector_and_missing_required_config` (404 names the connector + points at /catalog; 400 lists `base_url, path` for a bare `rest`; nothing persisted). Existing suites updated where they (correctly) tripped the gate: two weir-app unit tests now point WEIR_CONNECTORS_DIR at the testkit, `pinned()` uses the live connectors dir, `cli.rs::per_side_config_round_trips` stages connectors.
- **Side effect worth knowing:** `weir-wasm-testkit::connectors_dir()` now stages the PRODUCTION connector set (rest, rest-dest, snowflake, postgres, mssql — mirroring `scripts/stage-connectors.sh`) in addition to the fixtures, because tests that create connections referencing them must resolve. First cold run builds those guests (~minutes; freshness-cached after); CI without guest-target caching pays it per run — worth a cache tweak if CI time grows. This also sets up [[WEIR-T-0172]]'s "one list, glob-driven" goal.
- Deliberately skipped: the optional creation-time warning for unresolvable `{{ config['key'] }}` template keys (needs manifest-aware key extraction — belongs with [[WEIR-I-0033]]'s manifest compiler).
- Verified: `angreal test unit` green (all binaries), `weir-api` api suite 16/16, `weir-app` cli 8/8 + serve 2/2, `angreal test manifests` green, `angreal check all` clean.

**2026-08-28 — post-review fix (pre-v0.0.1 release review).** The adversarial release review confirmed a regression in this gate: `validate_required_config` loaded the spec via the UN-scoped ref, and the resolver's fallback runs tenant-dir → shared-dir only — so a tenant-PRIVATE compiled artifact (staged solely under `<dir>/<tenant>/<pkg>` by `POST /catalog/import`) passed `validate_connector_resolves` but then failed spec-load, 4xx-ing a connection that would run fine. Fixed in `add_connection` by scoping the refs with `scope_wasm_to_tenant` before the required-config check (validation now mirrors execution's resolution exactly). Regression test: new binary `crates/weir-app/tests/tenant_private.rs` — acme-private-only artifacts pass creation validation; a tenant without the artifact is still refused. 1/1 green; weir-app lib 28/28.
