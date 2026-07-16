---
id: host-side-snowflake-key-pair-jwt
level: task
title: "Host-side Snowflake key-pair JWT auth scheme"
short_code: "WEIR-T-0156"
created_at: 2026-07-15T02:08:33.996351+00:00
updated_at: 2026-07-15T22:48:18.893572+00:00
parent: WEIR-I-0041
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0041
---

# Host-side Snowflake key-pair JWT auth scheme

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0041]]

## Objective **[REQUIRED]**

Host-side credential scheme for Snowflake key-pair auth: build the self-signed RS256 JWT
(`iss/sub = ACCOUNT.USER` + SHA256 public-key fingerprint), inject as
`Authorization: Bearer` + `X-Snowflake-Authorization-Token-Type: KEYPAIR_JWT`, re-mint before the ≤1h expiry.
Secrets (private key PEM) stay host-side per [[WEIR-A-0033]]. Unlocks [[WEIR-T-0157]]/[[WEIR-T-0158]].

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria **[REQUIRED]**

- [ ] A manifest can declare `auth_scheme: snowflake_keypair_jwt`; the bundle (SOPS slug `snowflake`,
      [[WEIR-S-0018]]) supplies account identifier, user, and private key PEM.
- [ ] JWT claims match Snowflake's spec (uppercased account/user, fingerprint `SHA256:` prefix); token re-minted
      ahead of expiry; the extra token-type header is injected alongside the bearer.
- [ ] Key/token never reach guest config or logs (same assertion style as [[WEIR-T-0155]]).
- [ ] Unit-tested claim construction against known-answer fixtures.

## Implementation Notes **[CONDITIONAL: Technical Task]**

### Technical Approach
Same credential-path seam as [[WEIR-T-0155]] — these two schemes should land together so the second one proves
the seam generalizes. Needs an extra-static-header capability on the credential (the token-type header), which
the existing bearer injection may not have yet.

### Dependencies
None to start; blocks [[WEIR-T-0157]] and [[WEIR-T-0158]].

### Risk Considerations
Fingerprint derivation (DER vs PEM encoding of the public key) is the classic failure point — verify against a
live `SELECT CURRENT_USER()` as soon as the trial account exists.

## Status Updates **[REQUIRED]**

### 2026-07-15 — design fixed; implementing

Two Snowflake-specific findings vs the T-0155 pattern:
1. **Self-signed** — no token endpoint; the RS256 JWT *is* the bearer. `mint()` is a local sign (no
   `run_blocking`/network); cache per (account, user) in the shared process-wide map, 1h TTL, 60s margin.
2. **Fingerprint is over SPKI DER** — ring exposes the public key as PKCS#1; hand-roll the small SPKI DER
   wrap (rsaEncryption AlgorithmIdentifier + BIT STRING) rather than adding the `rsa` crate. Known-answer from
   the fixture key via openssl: `SHA256:BgiJGcINPQuBjB0MFoHV/SW8vP5pzN90TXJe8SpDZtI=`.

Plan: refactor `sa_assertion` internals into shared `parse_pkcs8_keypair`/`rs256_sign_jwt`; new
`SnowflakeKeypairProvider` (claims `iss=ACCOUNT.USER.SHA256:<fp>`, `sub=ACCOUNT.USER`, uppercased) + `apply`
injects `Authorization: Bearer` **and** `X-Snowflake-Authorization-Token-Type: KEYPAIR_JWT`. Config keys:
`snowflake_account_key`/`snowflake_user_key`/`snowflake_private_key_key` (defaults account/user/private_key);
**only the private key is stripped** — account/user stay in guest config (needed for `{{ config['account'] }}`
url_base templating in T-0157/58). Manifest `Auth::SnowflakeKeypairJwt`, both weir-app emitters, importer
`SnowflakeKeypairAuthenticator` extension. Tests: known-answer fingerprint + claims (fixed iat), parse/strip,
both-headers injection, engine e2e (no token mock needed — assert both headers on the captured request).

### 2026-07-15 — all green; ready for review

- Implemented as planned. Shared refactor landed: `cached_bearer` (process-wide `MINTED_TOKENS`, scheme-prefixed
  keys), `parse_pkcs8_keypair`, `rs256_sign_jwt`; the google provider now rides the same helpers — the seam
  generalizes, as the task intended.
- **SPKI fingerprint verified against an independent openssl known answer** (the task's flagged risk):
  `fingerprint_matches_openssl_spki_known_answer` green — hand-rolled PKCS#1→SPKI DER wrap is correct.
- Suites: weir-runtime 16/16 (snowflake: fingerprint KA, claims w/ fixed iat, uppercasing + self-sign cache,
  strip-only-private-key, both-headers `apply` unit test via a real `http_types` request); importer 18 (new
  lowering test); weir-app manifest tests 6; engine `wasm_http_engine` 22/22 incl. the snowflake e2e (bearer +
  token-type header on the captured request; account/user retained in sanitized guest config, key stripped).
  fmt + clippy clean.
- **Drive-by flake fix (pre-existing):** `egress_tests::oauth_provider_mints_and_caches_bearer` raced its
  single-`read` mock (respond-before-body-read → RST → intermittent EINVAL). Applied the same
  read-full-request fix the engine tests use, to both that mock and the session-provider one. Runtime suite
  10×/10× green after.
- Live verification against a real trial account (the fingerprint acceptance sanity `SELECT CURRENT_USER()`)
  lands with [[WEIR-T-0157]]/[[WEIR-T-0158]] once the `snowflake` bundle exists ([[WEIR-S-0018]] §2).

Awaiting human review + transition to completed.
