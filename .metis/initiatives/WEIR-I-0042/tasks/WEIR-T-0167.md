---
id: ui-error-surfacing-stop-swallowing
level: task
title: "UI error surfacing — stop swallowing HTTP failures, show server reasons in toasts"
short_code: "WEIR-T-0167"
created_at: 2026-08-16T15:24:04.507690+00:00
updated_at: 2026-08-16T15:24:04.507690+00:00
parent: WEIR-I-0042
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/todo"


exit_criteria_met: false
initiative_id: WEIR-I-0042
---

# UI error surfacing — stop swallowing HTTP failures, show server reasons in toasts

## Parent Initiative

[[WEIR-I-0042]]

## Objective **[REQUIRED]**

The UI erases every failure: `get_json` returns `T::default()` on ANY HTTP error, so a 401/500/dead server renders as "No connections yet" and empty dashboards; failure toasts are generic and never carry the server's error body. Surface errors as errors: distinct error states, server reasons in toasts, 401 → sign-in gate.

## Evidence (2026-08-16 alpha review)

- `weir-ui/src/main.rs:220-225` — `get_json` default-on-any-failure (verified).
- `weir-ui/src/main.rs:490-491, 1299-1301` — toasts: generic text, never auto-dismiss, not closable.
- Good backend messages exist and are unreachable from the UI (e.g. mode-validation messages in `crates/weir-app/src/lib.rs:1844+`).
- Review rated this the single worst "silently empty dashboard" trap for first-time users.

## Acceptance Criteria **[REQUIRED]**

- [ ] The fetch layer distinguishes success / 401 / other 4xx / 5xx / network failure; dashboards render an explicit error state (not empty-state copy) when calls fail
- [ ] 401 routes to the sign-in gate instead of rendering empty data
- [ ] Mutation-failure toasts include the server's message (status + body), e.g. the [[WEIR-T-0166]] validation reasons appear verbatim
- [ ] Toasts auto-dismiss and/or are closable
- [ ] Test coverage: server-down → error banner; invalid create → server reason visible (e2e or component-level)

## Implementation Notes

The whole UI is one 1511-line `weir-ui/src/main.rs` — keep the change surgical: a Result-returning fetch wrapper plus an error signal consumed by the shell/views; do not restructure the file here. The 800ms polling loop should back off or show a degraded banner on repeated failure rather than hammering a dead server. Coordinate with [[WEIR-T-0166]] so creation-time 4xx bodies surface verbatim.

## Status Updates **[REQUIRED]**

*To be added during implementation*
