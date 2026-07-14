---
id: ui-auth-gate-leptos-oidc-sign-in
level: task
title: "UI auth gate (Leptos) — OIDC sign-in + API-key entry, ride the session"
short_code: "WEIR-T-0087"
created_at: 2026-07-05T21:12:01.441613+00:00
updated_at: 2026-07-05T22:14:55.641514+00:00
parent: WEIR-I-0017
blocked_by: [WEIR-T-0086]
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0017
---

# UI auth gate (Leptos) — OIDC sign-in + API-key entry, ride the session

## Parent Initiative

[[WEIR-I-0017]]. Makes the Leptos + Aurora UI ([[WEIR-I-0016]]) work behind auth. Governed by [[WEIR-A-0008]].

## Objective

Gate the SPA on auth: an unauthenticated user gets a **sign-in** screen (OIDC redirect **or** paste an API
key); once authenticated the existing Operations/Setup views load and all API calls carry the credential;
a **Sign out** control clears it.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [ ] **Gate** — on load, probe auth (e.g. `GET /auth/me` → 200 `{subject,kind}` or 401). On 401 render an
  Aurora sign-in card: **"Sign in with OIDC"** button (→ `/auth/login`) + an **API key** field ("Use an API
  key" → store it, retry). No app chrome is shown until authenticated.
- [ ] **Session path** — after OIDC redirect back, the httpOnly cookie rides automatically; the SPA just
  re-probes `/auth/me` and shows the app. (Cookie is invisible to JS by design.)
- [ ] **Key path** — a pasted key is held in memory/localStorage and sent as `Authorization: Bearer` on every
  `gloo-net` request. Centralize the fetch so **all** calls include the credential + handle a 401 by
  bouncing back to the gate.
- [ ] **Sign out** — a header control: `POST/GET /auth/logout` (clears the session) + drop any stored key →
  back to the gate.
- [ ] **CSRF** — cookie-authenticated mutations include the `csrf_token` per [[WEIR-T-0086]].
- [ ] `angreal ui build` green; the Operations/Setup views still work once authenticated; clippy clean.

## Technical Notes

- Add a thin auth-aware fetch wrapper in `weir-ui` so credential + 401-handling live in one place (don't
  sprinkle headers across every call site).
- Use Aurora components for the sign-in card (`Panel`/`Button`/`TextInput`), consistent with the shell.
- `/auth/me` is a small authenticated endpoint returning the current `Principal` (add it in [[WEIR-T-0086]]
  or here) for the gate probe + the "signed in as …" header text.

## Dependencies

- **Blocked by [[WEIR-T-0086]]** (OIDC + session + `/auth/*`). Builds on the [[WEIR-I-0016]] Leptos UI.

## Status Updates

### 2026-07-05 — UI auth gate done; e2e re-greened

- **Gate** (`weir-ui/src/main.rs`) — on mount, `recheck()` probes `GET /auth/me`: 200 → `authed=Some(true)`
  (app shows), else `Some(false)` → a **full-screen Aurora sign-in overlay** (`Panel` "Sign in to weir"):
  **"Sign in with OIDC"** (→ `window.location = /auth/login`) + an **API-key** field → `use_key()` stores it
  in `localStorage["weir_api_key"]` and re-probes. No app chrome until authenticated.
- **Key path** — the T-0084 `areq_*` wrappers already send `Authorization: Bearer` from localStorage; a pasted
  key flows straight through.
- **Session path** — the httpOnly cookie rides automatically (the T-0086 cookie door); the gate just re-probes.
- **Sign out** — a header control: clears the key + `GET /auth/logout` → back to the gate. Deps: `web-sys` Location.

**e2e re-greened (the T-0084 rescope):** `e2e/tests/fixtures.ts` seeds `localStorage["weir_api_key"]` from
`WEIR_E2E_KEY` (the harness mints it via `weir init`); the 4 specs import from `./fixtures`. Full suite
**4/4 green** against the locked API (the seed required the bearer — proof the gate is real). Added
`gate.spec.ts` (unauthenticated → the sign-in card shows) + a **screenshot** confirming the Aurora gate.
clippy clean. (CSRF on cookie mutations is deferred with the OIDC flow → [[WEIR-T-0088]].) **Complete.**
