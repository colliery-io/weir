---
id: host-side-google-service-account
level: task
title: "Host-side Google service-account JWT auth scheme"
short_code: "WEIR-T-0155"
created_at: 2026-07-15T02:08:27.641742+00:00
updated_at: 2026-07-15T22:17:26.888798+00:00
parent: WEIR-I-0041
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0041
---

# Host-side Google service-account JWT auth scheme

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0041]]

## Objective **[REQUIRED]**

A new host-side credential scheme ([[WEIR-A-0033]]/[[WEIR-A-0037]]): given a Google service-account JSON key,
the host builds the RS256 JWT assertion, exchanges it at `oauth2.googleapis.com/token` for an access token
(scoped per connection), caches it, refreshes before expiry, and injects it as a bearer header. The guest never
sees the key or the token. Unlocks GA4 ([[WEIR-T-0159]]) and Sheets ([[WEIR-T-0160]]).

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria **[REQUIRED]**

- [ ] A manifest can declare `auth_scheme: google_service_account` with a scope list; the secret bundle supplies
      the SA JSON key (SOPS slug `google`, [[WEIR-S-0018]]).
- [ ] Token minted via JWT-bearer grant, cached per (key, scopes), refreshed ahead of the 1h expiry; concurrent
      streams share one token.
- [ ] Key material and access token never appear in guest config, logs, or the manifest — asserted by a test on
      the egress/credential path.
- [ ] Unit tests cover assertion construction + refresh; a mocked token-endpoint test covers the exchange.

## Implementation Notes **[CONDITIONAL: Technical Task]**

### Technical Approach
Follow the Salesforce OAuth2 host-mint pattern from [[WEIR-T-0075]] (rest-dest) but land it in the shared
credential path (`Credential::from_auth_config` seam) so both `rest` and `rest-dest` get it. Note this widens
the `auth_scheme` string vocabulary tracked as debt in [[WEIR-I-0033]]/[[WEIR-I-0034]] — add the scheme in one
place if the typed schema lands first.

### Dependencies
None to start; consumed by [[WEIR-T-0159]] and [[WEIR-T-0160]].

### Risk Considerations
Clock skew on the JWT `iat/exp` claims; Google rejects assertions with >5min skew — use server-fetched time or
tolerate with a short validity window.

## Status Updates **[REQUIRED]**

### 2026-07-15 — implemented; pending fixture keys + test run

**Seam findings:** `Credential::from_auth_config` (crates/weir-runtime/src/lib.rs ~L297) is the single host-side
scheme registry; `HostAllowList::authorize` → `Credential::apply` injects per request. `OAuth2Provider` is the
mint/cache pattern (`run_blocking` ureq on a dedicated thread — wasmtime executor threads EINVAL on blocking
sockets; `REFRESH_MARGIN` 60s). `ring 0.17` already in tree via rustls → RS256 with no new heavyweight dep.

**Implemented:**
- `weir-manifest`: `Auth::GoogleServiceAccount { key_key (default service_account_key), scopes }`.
- `weir-importer`: `GoogleServiceAccountAuthenticator` extension type (weir-specific — Airbyte's spec has no
  Google SA authenticator) → lowers to the manifest Auth; counted as supported in `analyze()`. This makes
  [[WEIR-T-0159]]/[[WEIR-T-0160]] pure manifest authoring as intended. Test: `imports_google_service_account_authenticator`.
- `weir-app`: both `manifest_stream_to_config` and `dest_object_to_config` emit `auth_scheme:
  google_service_account` + `google_sa_key_key` + `google_scopes` (non-secret metadata only). Test added.
- `weir-runtime`: `Credential::GoogleServiceAccount(GoogleSaProvider)`; from_auth_config arm accepts the SA
  JSON as object **or** JSON-encoded string, strips it + metadata; `sa_assertion()` pure RS256 JWT builder
  (ring, PKCS#8 PEM→DER); mint via `urn:ietf:params:oauth:grant-type:jwt-bearer` form POST; **process-wide**
  token cache keyed `(client_email, scopes)` with the shared 60s refresh margin. Unit tests: assertion
  header/claims, parse+strip (non-leak), mocked-endpoint mint+cache, refresh-inside-margin.
- Engine e2e test `wasm_http_source_google_sa_host_mints_and_injects_bearer` (mirrors the oauth2 one): asserts
  the signed grant hits the token mock, bearer injected on the guest request, and key material absent from the
  sanitized guest config.

### 2026-07-15 — all green; ready for review

- Fixture keys generated (throwaway 2048-bit PKCS#8, one copy per crate's tests/fixtures).
- weir-runtime lib: 11/11 (assertion claims, parse+strip non-leak, mocked JWT-bearer mint + cache-hit,
  refresh-inside-margin). weir-importer: 17 lib + 2 fidelity. weir-manifest: 6 (18 total w/ parse suite).
  weir-app manifest tests: 5/5.
- Engine e2e `wasm_http_source_google_sa_host_mints_and_injects_bearer` green; full `wasm_http_engine` 21/21.
- Manifest corpus still 34/34 tier A (importer change is additive). `cargo fmt` + `angreal check all` clean.

**AC check:** (1) manifest declares the scheme + scopes; SA JSON via config key (SOPS `google` bundle shape
supported as object or string) ✓. (2) JWT-bearer mint, process-wide cache per (client_email, scopes), 60s
refresh margin ahead of the 1h expiry ✓. (3) key/token never in guest config — asserted in unit + engine tests
on the sanitized-config/egress path; never logged (errors carry no key material) ✓. (4) unit tests for
assertion + refresh, mocked token-endpoint exchange ✓.

Awaiting human review + transition to completed.
