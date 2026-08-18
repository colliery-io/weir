---
id: validate-connector-existence
level: task
title: "Validate connector existence + config shape at POST /connections (4xx, not 201-then-fail)"
short_code: "WEIR-T-0166"
created_at: 2026-08-16T15:24:03.074650+00:00
updated_at: 2026-08-16T15:24:03.074650+00:00
parent: WEIR-I-0042
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/todo"


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

- [ ] POST /connections (and the tenant-scoped variant) with an unknown source or dest → 404/422 naming the connector and pointing at the catalog endpoints for valid names
- [ ] Config missing requireds per the connector's config_schema → 422 with a field-level reason
- [ ] Upsert-by-name semantics for valid payloads unchanged; `crates/weir-api/tests/api.rs` covers both rejection paths
- [ ] Error bodies are human-useful — pairs with [[WEIR-T-0167]] so users actually see them in the UI

## Implementation Notes

The existence check must consult the same resolution order the executor uses (tenant-scoped artifact dir + shared connectors dir + catalog registrations — see `scope_wasm_to_tenant` and the ingress staging paths) or it will reject connectors that would in fact run. Mode/cadence validation already exists in weir-app (`validate_connection_modes` area, ~L1844) — this extends the same creation-time gate, not a new mechanism.

## Status Updates **[REQUIRED]**

*To be added during implementation*
