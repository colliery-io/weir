---
id: session-provider-credentialed
level: task
title: "Session provider — credentialed login bodies + 401-driven re-login"
short_code: "WEIR-T-0180"
created_at: 2026-08-29T14:26:37.627060+00:00
updated_at: 2026-08-29T20:03:16.979802+00:00
parent: WEIR-I-0043
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0043
---

# Session provider — credentialed login bodies + 401-driven re-login

## Parent Initiative

[[WEIR-I-0043]]

## Objective **[REQUIRED]**

Harden the host-side **session** credential provider (`Credential::Session` in `crates/weir-runtime/src/lib.rs`, the ~L1104 follow-up): support credentialed login request BODIES (username/password/JSON shapes, not just a bare POST) and re-login automatically when the upstream starts answering 401 — so session-token APIs survive token expiry mid-sync instead of failing the run.

## Scope (deliberately bounded by [[WEIR-I-0043]] non-goals)

- Login request: configurable method + JSON body template fed from the connection's secret config (host-side only, [[WEIR-A-0033]] — the secret never enters the guest); token extracted at the configured response path (existing mechanism).
- 401-driven re-login: on a 401 from the target API, drop the cached token, re-run login once, replay the request; a second 401 fails the request (no retry loops).
- NOT in scope: refresh-token flows, OAuth (deferred per the initiative), cookie jars, CSRF dances — those are provider work beyond "credentialed login + re-login".

## Acceptance Criteria **[REQUIRED]**

*(Amended 2026-08-29 during implementation: in-run 401-intercept-and-replay is ARCHITECTURALLY INEXPRESSIBLE weir-side — fidius's `EgressHooks` dispatches via `default_send_request` and hands the response future straight back to the guest; the embedder's `EgressPolicy` sees requests only. The 401 AC is replaced by (a) proactive TTL re-login, which prevents the expiry instead of reacting to it, and (b) the documented run-level replay: a mid-run 401 fails the attempt and the worker retry ([[WEIR-T-0169]]) re-resolves the credential — a fresh login — and resumes from the checkpoint. True in-run replay would need a fidius response-hook FR — noted for the backlog, same seam family as FIDIUS-I-0034.)*

- [x] A session provider config can declare a login body (`session_login_body` JSON template; `{{key}}` values substituted from secret config fields); the login fires host-side and the token injects per request as today
- [x] Token expiry mid-sync is survivable: `session_ttl_secs` re-logs-in proactively BEFORE the token expires (mock-HTTP unit test proves body delivery + cache + post-ttl rotation); the 401 fallback path (run retry → fresh login → checkpoint resume) is documented
- [x] Secrets stay host-side: substituted body keys and all session metadata are stripped from the sanitized guest config (unit test asserts no secret/metadata leakage)
- [x] `angreal check all` + unit wall green; docs (connection-config reference) describe the session scheme's keys

## Status Updates **[REQUIRED]**

**2026-08-29 — implemented + verified (ralph run); 401 AC amended per the seam finding above.**

- `SessionProvider` (`crates/weir-runtime/src/lib.rs`) gains `login_method` (default POST), `login_body` (JSON string, sent with `content-type: application/json`), and `ttl` with a `(token, minted_at)` cache — a cached token older than ttl is re-minted before use; zero = cache for the provider's lifetime (one run), the prior behavior.
- `Credential::from_auth_config` session arm: `session_login_body` is a JSON object template whose `"{{key}}"` string values are substituted from the config, with each referenced key pushed onto the strip list ([[WEIR-A-0033]] — secrets never reach the guest); `session_login_method` + `session_ttl_secs` parsed; all three new keys always stripped.
- Tests (weir-runtime lib 23/23): `session_provider_sends_login_body_and_relogs_in_after_ttl` (mock server captures the request: method+content-type+exact body; cached inside ttl; token rotates after ttl) and `session_login_body_template_strips_secret_fields` (secret values, secret key names, and the body template all absent from the sanitized config; harmless keys remain).
- Docs: `docs/reference/connection-config.md` gains a "Host-side auth (`auth_scheme`)" section listing every scheme + a session key table incl. the ttl semantics and the 401→retry→checkpoint-resume behavior.
- `angreal check all` clean; unit wall 12/12.
