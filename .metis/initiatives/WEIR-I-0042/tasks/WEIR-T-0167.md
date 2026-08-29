---
id: ui-error-surfacing-stop-swallowing
level: task
title: "UI error surfacing — stop swallowing HTTP failures, show server reasons in toasts"
short_code: "WEIR-T-0167"
created_at: 2026-08-16T15:24:04.507690+00:00
updated_at: 2026-08-25T02:55:04.604603+00:00
parent: WEIR-I-0042
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


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

- [x] The fetch layer distinguishes success / 401 / other 4xx / 5xx / network failure; dashboards render an explicit error state (the `.weir-apierr` banner over last-known data) when calls fail
- [x] 401 routes to the sign-in gate instead of rendering empty data
- [x] Mutation-failure toasts include the server's message — the [[WEIR-T-0166]] validation reasons appear verbatim (e2e-proven)
- [x] Toasts auto-dismiss (6s, sequence-guarded) and are click-to-dismiss
- [x] Test coverage: `e2e/tests/errors.spec.ts` — route-abort → banner over persisting data → clears on recovery; invalid create → server reason in toast

## Implementation Notes

The whole UI is one 1511-line `weir-ui/src/main.rs` — keep the change surgical: a Result-returning fetch wrapper plus an error signal consumed by the shell/views; do not restructure the file here. The 800ms polling loop should back off or show a degraded banner on repeated failure rather than hammering a dead server. Coordinate with [[WEIR-T-0166]] so creation-time 4xx bodies surface verbatim.

## Status Updates **[REQUIRED]**

**2026-08-25 — implemented; e2e verification in flight (ralph run).**

- Fetch layer (`weir-ui/src/main.rs`): new `Fetched<T>` enum (Ok / Unauthorized / Failed(status, server message) / Network) + `get_fetch`, `server_error` (extracts the API's uniform `{"error": …}` body), and `check` (classifies mutation responses into a human reason carrying the server's message). `get_json` retained only for non-critical detail fetches.
- Poll loop: `/connections` is the canary — 401 flips the sign-in gate; HTTP/network failures set an `api_error` signal; every fetch only overwrites its signal on success, so an outage shows the banner over **last-known data** instead of fake empty states; the loop backs off 800ms → 3.2s while degraded.
- New fixed top banner (`.weir-apierr`) renders while `api_error` is set: "⚠ control plane error — {reason}".
- Toasts: auto-dismiss after 6s (sequence-guarded against replacing a newer toast), click-to-dismiss, and every failure toast now carries the server's reason — all ten mutation sites updated (run/delete/start/stop, onboard-pick, preview, onboard-byo, save-connection, create-tenant, mint-key, revoke-key, accept-schema), so the [[WEIR-T-0166]] validation messages surface verbatim.
- New `e2e/tests/errors.spec.ts`: (1) rejected create → toast contains "Couldn't save …" + "unknown source connector"; (2) `page.route` aborts `/connections` → the degraded banner appears while the seeded fx-demo card KEEPS rendering (no fake empty state), then clears on unroute.
- UI compiles clean for wasm32-unknown-unknown. **Verified 2026-08-25:** `angreal test e2e` — 12/12 passed (both new error specs + all pre-existing, incl. the real OIDC/Dex round-trip). The e2e server's fresh store also re-proved the [[WEIR-T-0164]] bootstrap banner in passing.
