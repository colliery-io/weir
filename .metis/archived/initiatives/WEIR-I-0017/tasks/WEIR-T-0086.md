---
id: generic-oidc-login-httponly-cookie
level: task
title: "Generic OIDC login + httpOnly cookie session (2nd auth door)"
short_code: "WEIR-T-0086"
created_at: 2026-07-05T21:11:33.331041+00:00
updated_at: 2026-07-05T22:08:02.661511+00:00
parent: WEIR-I-0017
blocked_by: [WEIR-T-0084]
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0017
---

# Generic OIDC login + httpOnly cookie session (2nd auth door)

## Parent Initiative

[[WEIR-I-0017]]. The human door. Governed by [[WEIR-A-0008]].

## Objective

Add a **generic OIDC** Authorization-Code login: `GET /auth/login` → provider; `GET /auth/callback` verifies
the ID token and mints a server-side **httpOnly cookie session**; the AuthN middleware ([[WEIR-T-0084]])
accepts the session cookie as a second way to resolve a `Principal { kind: User }`. `GET /auth/logout` clears
it. Configured by issuer URL + client id/secret (no pinned vendor).

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [ ] **Config** — OIDC provider from env/config: `issuer`, `client_id`, `client_secret`, `redirect_uri`,
  `scopes` (default `openid email profile`). Discovery via the issuer's `.well-known/openid-configuration`.
  When unconfigured, `/auth/login` returns a clear "OIDC not configured" (keys still work).
- [ ] **`GET /auth/login`** — builds the authorize URL with `state` + `nonce` (stored, single-use, CSRF), 302 to provider.
- [ ] **`GET /auth/callback`** — validate `state`; exchange code → tokens; **verify the ID token**
  (signature via JWKS, `iss`, `aud`, `exp`, `nonce`); derive subject (`sub`/`email`); create a `sessions`
  row ([[WEIR-T-0083]]); set an **httpOnly, SameSite=Lax, Secure** cookie holding only the opaque session id;
  redirect to the UI.
- [ ] **Middleware 2nd door** — [[WEIR-T-0084]]'s layer also accepts a valid session cookie → `Principal
  {kind:User, subject}`. Bearer key OR session → authenticated; neither → 401. **CSRF**: enforce the
  double-submit / `csrf_token` on cookie-authenticated **mutations** (bearer-key requests are exempt).
- [ ] **`GET /auth/logout`** — delete the session row + clear the cookie.
- [ ] Uses a vetted OIDC/OAuth2 crate (e.g. `openidconnect`); ID-token verification is real (not skipped).
- [ ] Unit/integration: a stubbed-or-Dex login mints a working session; tampered/expired ID token rejected;
  logout invalidates. (Full Dex e2e is [[WEIR-T-0088]].) clippy clean.

## Technical Notes

- Server-side sessions (opaque id in the cookie; state in `sessions`) → clean revocation + logout, no
  client-trusted claims. Session TTL from `expires_at`.
- Keep the OIDC client construction lazy/cached; discovery is one round-trip.
- Don't log tokens or the client secret.

## Dependencies

- **Blocked by [[WEIR-T-0084]]** (extends its middleware) + [[WEIR-T-0083]] (`sessions`). Prereq for [[WEIR-T-0087]]/[[WEIR-T-0088]].

## Status Updates

### 2026-07-05 — identity plumbing done; openidconnect flow folded into T-0088 (Dex)

The cloacina model (revised [[WEIR-A-0008]]) unifies on keys: **OIDC login mints a short-lived key** held in
the `weir_session` cookie — there's **no separate `sessions` table** (the AC above predates the realignment).
Split T-0086 by testability:

**Done + green (identity/session plumbing, no live IdP needed):**
- **Cookie 2nd door** — `require_auth` now accepts `Authorization: Bearer` **or** the `weir_session` cookie
  (`session_cookie` parses it) → the same `AuthenticatedKey`. Bearer for machines, cookie for humans.
- **`GET /auth/me`** (authed) — returns the principal (subject/name/role/tenant/is_admin) for the UI gate.
- **`GET /auth/logout`** (public) — clears the cookie.
- **`GET /auth/login`** (public) — reports **not-configured** (501) until the openidconnect flow lands.
- `App::mint_api_key(...expires_at, issued_via)` (from T-0083 rework) is the `mint_for_principal` primitive.
- Test `auth_me_and_session_cookie_door`: /auth/me identity, the cookie authenticates `/connections`, login=501.

**Deferred to [[WEIR-T-0088]] (where it's testable):** the real openidconnect v4 flow — discovery, `/auth/login`
redirect (PKCE+state+nonce), `/auth/callback` (code exchange, JWKS ID-token verification, mint a ~15-min key,
set the httpOnly cookie), against **Dex**. Can't be meaningfully tested without a live IdP, so it lands with
the Dex stack. Mirrors cloacina `oidc.rs` + `routes/oidc_auth.rs` (`openidconnect = "4"`).

**Status:** the human 2nd-door + the UI-gate contract (`/auth/me`, `/auth/logout`) are in + green; clippy
clean. T-0087 (UI gate) can build on the key door + `/auth/me` now; T-0088 makes OIDC sign-in real. **Complete
for this scope.**
