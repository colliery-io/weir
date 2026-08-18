---
id: flagship-manifest-fidelity
level: task
title: "Flagship manifest fidelity — pagination (+incremental) for stripe/hubspot"
short_code: "WEIR-T-0168"
created_at: 2026-08-16T15:24:06.182973+00:00
updated_at: 2026-08-16T15:24:06.182973+00:00
parent: WEIR-I-0042
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/todo"


exit_criteria_met: false
initiative_id: WEIR-I-0042
---

# Flagship manifest fidelity — pagination (+incremental) for stripe/hubspot

## Parent Initiative

[[WEIR-I-0042]]

## Objective **[REQUIRED]**

The two flagship SaaS manifests silently truncate: stripe.yaml and hubspot.yaml omit pagination (with in-file "⚠ unconfirmed" notes) although the shared declarative runtime supports opaque-cursor paging — a Stripe sync returns ~10 records and reports success, which is worse than failing. Add pagination (+ incremental where the API supports it) so the flagship manifests are correct, and sweep the other unpaginated manifests.

## Evidence (2026-08-16 alpha review)

- `manifests/stripe.yaml:40-46`, `manifests/hubspot.yaml:26-28` — no paginator, "⚠ unconfirmed" field notes; Stripe also lacks incremental.
- Runtime support already exists: opaque-cursor + POST-body param injection in `crates/connectors/rest/src/lib.rs`.
- Also unpaginated per the connectors survey: airtable, todoist, openweather.
- Constraint to verify: opaque-cursor tokens must be JSON strings (`.as_str()`, `crates/connectors/rest/src/lib.rs:698-703`) and there is NO has_more-boolean stop-condition support — HubSpot's `paging.next.after` is a string (fine); Stripe pages by `starting_after` = last object id + `has_more`, which may need a small runtime/importer addition.

## Acceptance Criteria **[REQUIRED]**

- [ ] stripe.yaml pages through full result sets — mechanism verified against Stripe's actual API shape; any runtime gap (has_more stop condition, id-as-cursor) is either implemented in the shared runtime or filed with evidence — and has an incremental cursor where valid (Stripe's `created` filter model; document what's expressible)
- [ ] hubspot.yaml pages via the opaque `paging.next.after` cursor
- [ ] airtable / todoist / openweather checked; paginators added where their APIs page
- [ ] "⚠ unconfirmed" annotations resolved or corrected; `angreal test manifests` green; a mock-HTTP engine test covers at least the Stripe-shaped pagination pattern
- [ ] Live verification explicitly recorded as pending the [[WEIR-T-0067]] secret bundles — do not claim live-verified

## Implementation Notes

If a runtime addition is needed for Stripe (cursor-from-last-record-id and/or has_more stop condition), add it to the shared runtime in `crates/connectors/rest` with a mock-HTTP engine test, mirror it in the importer's construct mapping, and note it in the declarative parity ledger (WEIR-S-0016). Watch the interaction with the whole-stream-in-memory read: full pagination on a large Stripe account will buffer everything — acceptable for alpha, but note it; MAX_PAGES=1000 still caps.

## Status Updates **[REQUIRED]**

*To be added during implementation*
