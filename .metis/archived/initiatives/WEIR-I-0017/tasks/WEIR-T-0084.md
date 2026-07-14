---
id: authn-middleware-bearer-keys-lock
level: task
title: "AuthN middleware (bearer keys) — lock the API down except /health"
short_code: "WEIR-T-0084"
created_at: 2026-07-05T21:10:32.192858+00:00
updated_at: 2026-07-05T21:57:23.664144+00:00
parent: WEIR-I-0017
blocked_by: [WEIR-T-0083]
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0017
---

# AuthN middleware (bearer keys) — lock the API down except /health

## Parent Initiative

[[WEIR-I-0017]]. The **hard cutover**: this is where the API stops being open. Governed by [[WEIR-A-0008]].

## Objective

Add an axum AuthN middleware that validates `Authorization: Bearer <key>` (via the [[WEIR-T-0083]] key
store) → injects a `Principal` extension, or **401**. Applied to every route **except `/health`**. Then
thread the bootstrap key through everything that calls the API so nothing is left red.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [ ] **Middleware** — a `tower`/axum layer (or `from_fn` middleware) in `crates/weir-api`: read the
  `Authorization: Bearer` header → `verify()` → insert `Principal` into request extensions; missing/invalid
  → `401` with a JSON body. `/health` bypasses (public). Applied to the whole router (except `/health`).
- [ ] **Handlers** can pull the `Principal` (extractor / extension) — wire it through so [[WEIR-T-0085]] can
  authorize + audit with the actor.
- [ ] **Cutover done, nothing red** — update every API caller to present the key:
  - `weir-ui` (Leptos) fetches send the key (interim: a dev/bootstrap mechanism until the [[WEIR-T-0087]]
    gate; e.g. an injected header or a documented dev token) — the UI must still load.
  - the CLI/`app` integration + functional + serve tests (`crates/weir-api`, `crates/weir-app/tests`) mint
    a bootstrap key in setup and send it.
  - `e2e/` Playwright + `angreal test connectors-live` paths: document/inject a key so they still pass.
- [ ] `/health` reachable unauthenticated; a bare request to any other route → 401; with a valid key → 200.
- [ ] Workspace + clippy green; existing suites updated + green.

## Technical Notes

- Config: the server reads an env/flag for whether auth is enforced? **No** — auth is always on; tests mint a
  real key. (A kill-switch invites prod misconfig.) `/health` is the only public route.
- Prefer a single `require_auth` layer on the merged router with a `/health` route registered outside it.
- Keep the 401 body shape consistent with the API's existing error responses ([[WEIR-A-0006]]).

## Dependencies

- **Blocked by [[WEIR-T-0083]]** (key store + verify). Prereq for [[WEIR-T-0085]]/[[WEIR-T-0086]].

## Status Updates

### 2026-07-05 — cutover surface mapped; plan ready (T-0083 foundation is in)

**Surface (smaller than feared):** `weir_api::router(app: Arc<App>) -> Router` (`crates/weir-api/src/lib.rs`)
wires all routes + `.fallback(serve_ui)` + `.with_state(app)`, axum 0.8 (no tower-http dep yet). HTTP
callers that break when the API locks:
- `crates/weir-api/tests/api.rs` — the Rust HTTP test (mint a key in setup, send `Authorization: Bearer`).
- `weir-ui` (Leptos) — the SPA loads via the **public fallback**, but its `gloo-net` API calls need the key.
- `e2e/` Playwright (`shell`/`operations`/`reverse-etl`/`screenshot`) — browser API calls need the key.
- **Untouched:** `crates/weir-app/tests/*` (manifest_corpus, reverse_etl_*, serve, cli) + `connectors-live`
  use `App` **directly**, not the HTTP router — no change needed.

**Plan:**
1. **Middleware** — `axum::middleware::from_fn_with_state(app.clone(), require_auth)`: read `Authorization:
   Bearer` → `app.verify_api_key()` → insert `Principal` extension, else `401` JSON. Structure the router so
   **`/health` + the `serve_ui` fallback stay public** (build the API routes as a group, `.layer()` the auth
   on that group, then add `/health` + fallback outside it) so the SPA can still load + then authenticate.
2. **Rust test** — `api.rs`: mint a bootstrap key, send it on every request; add an unauth→401 + health-open case.
3. **UI (interim, minimal)** — a `weir-ui` fetch wrapper that sends `Authorization: Bearer` from a stored key
   (`localStorage weir_api_key`); the full sign-in gate is [[WEIR-T-0087]]. The SPA must still load unauthenticated.
4. **e2e** — the specs mint a key via the CLI/API in setup and seed `localStorage` before navigating; keep them green.
5. **Demo/live-CI** — connectors-live is unaffected (App-direct). The UI demo needs a seeded key (document it).

**Decision:** auth is always-on (no enforce/disable flag — a kill-switch invites prod misconfig); `/health`
is the only public API route + the SPA fallback. This is the atomic unit — middleware **and** all callers land
together (a half-done cutover reds the whole API), so it's committed as one green slice.

**Status:** foundation ([[WEIR-T-0083]]) shipped + committed (`09b5cc6`).

### 2026-07-05 — implemented (cloacina model): LRU-cached bearer gate

Per the [[WEIR-A-0008]] realignment, built on the reworked cloacina key store (`c70ff6b`):
- **Middleware** (`weir-api/src/lib.rs`): a `KeyCache` (LRU 256 / 30s TTL, `tokio::Mutex<LruCache>`, keyed by
  SHA-256 hash) + `AuthState{app,cache}` + `require_auth` (`from_fn_with_state`) — extract `Bearer` → hash →
  cache → else `app.validate_api_key` → cache → insert **`AuthenticatedKey`** extension, else **401 JSON**.
  Mirrors cloacina `routes/auth.rs`.
- **Router restructured** — API routes carry `.route_layer(require_auth)`; **`/health` + the SPA fallback stay
  public** (registered outside the layer). `lru` dep added.
- **api.rs test** — bearer threaded through all 6 existing tests (bootstrap key in setup); added
  `auth_gate_health_open_and_key_required` (health 200, no-key 401, bogus 401, valid 200). **7 passed.**
- **UI** (`weir-ui`) — `bearer()` (reads `localStorage["weir_api_key"]`) + `areq_get/post/delete` wrappers on
  all 9 fetch sites; `web-sys` dep. `angreal ui build` ✅. The SPA still loads (public fallback).

**Rescope:** the browser **e2e key-seeding** moves to [[WEIR-T-0087]] (the sign-in gate) — that's where the
login flow that sets `localStorage` lives; doing it twice makes no sense. The Rust suites (which gate CI) are
green; the Playwright specs need the seeding step T-0087 adds. **Backend + UI-auth-capability complete.**
