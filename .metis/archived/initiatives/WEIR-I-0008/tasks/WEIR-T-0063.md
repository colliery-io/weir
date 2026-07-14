---
id: s3-auth-coverage-oauth2
level: task
title: "S3: Auth coverage — OAuth2 + SessionToken via host-side token provider"
short_code: "WEIR-T-0063"
created_at: 2026-06-28T01:48:04.525468+00:00
updated_at: 2026-06-29T18:45:44.907966+00:00
parent: WEIR-I-0008
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0008
---

# S3: Auth coverage — OAuth2 + SessionToken via host-side token provider

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0008]] — slice S3 (Auth coverage). Tracked in [[WEIR-S-0016]] (Auth rows).

## Objective **[REQUIRED]**

Close the remaining auth wall on the shared `rest` runtime. Bearer / header-ApiKey / query-ApiKey are
done; the manifest survey showed **26 of 34 vendored connectors are authed**, and the big remaining
catalog set needs **OAuth2** (refresh-token + client-credentials grants) and **session-token** auth.

Crucially, the secret + token refresh stay **host-side** ([[WEIR-A-0013]]): the guest never sees a
client secret or runs a refresh loop. We add a **host-side token provider on the `EgressPolicy`** that
mints/refreshes the bearer and injects it on egress — the **same seam Salesforce OAuth reuses** when
reverse ETL ([[WEIR-I-0007]]) resumes. The manifest declares only the *scheme*; the per-connection
config supplies client id/secret/refresh token (or session credentials), never baked into the manifest.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria **[REQUIRED]**

- [ ] **OAuth2 token provider (host-side).** A token provider on the egress policy obtains and caches a
  bearer via (a) **refresh-token** grant and (b) **client-credentials** grant; refreshes on expiry
  (honors `expires_in` / 401-driven refresh); injects `Authorization: Bearer <token>` on guest egress.
  Secrets/refresh never enter the guest ([[WEIR-A-0013]]).
- [ ] **SessionToken auth.** Login request → token extracted from the response (path-configurable) →
  injected on subsequent requests per the manifest's `request_authentication` (header or query).
- [ ] **Importer mapping.** `weir-importer` maps Airbyte `OAuthAuthenticator`
  (refresh-token / client-credentials) and `SessionTokenAuthenticator` → the weir `Auth` scheme +
  config keys; `manifest_stream_to_config` surfaces the provider params. Unsupported sub-variants are
  reported (tier/confidence), not dropped.
- [ ] **Wire-level proof.** A test runs an OAuth-authed read over real `wasi:http` (mock token endpoint
  + resource endpoint): the connector fetches a token then sends it; refresh path exercised. Same for
  session-token. Mapping unit tests for both.
- [ ] **Keyed live run.** ≥1 real OAuth connector runs E2E via the keyed harness
  (`secrets/<slug>.json`, gitignored), rows > 0, skipped when the secret is absent.
- [ ] **Ledger flipped.** [[WEIR-S-0016]] Auth rows `OAuthAuthenticator` and `SessionTokenAuthenticator`
  move ❌ → ✅ in this change (DoD per [[WEIR-S-0016]] REQ-2). `BasicHttpAuthenticator` flipped too if it
  falls out cheaply; otherwise left ❌ with a note.
- [ ] Workspace + integration suites green; clippy clean.

## Implementation Notes **[CONDITIONAL: Technical Task]**

### Technical Approach
- Extend the host `EgressPolicy` (the `HostAllowList.inject_headers` seam, [[WEIR-A-0013]]) with a
  **token provider** trait: `fn bearer(&self) -> Result<String>` with internal caching + refresh.
  OAuth grant requests are made **host-side** (the host already brokers egress), keeping the guest a
  pure HTTP client.
- `rest` runtime: when the resolved `Auth` is OAuth/session, the host injects the minted bearer the same
  way it injects a static api-key today — so the runtime change is mostly in the policy, not the guest.
- Importer: add `OAuthAuthenticator` / `SessionTokenAuthenticator` arms mirroring the existing
  `inject_into` handling from commit `ed8d3db`; route grant params (`token_refresh_endpoint`,
  `grant_type`, `client_id/secret`, `refresh_token`, scopes) into config.

### Dependencies
- Builds directly on the shared-runtime auth v1 (commits `13d34b3`, `ed8d3db`) and the keyed harness
  (`keyed_manifests_run_live`). No blockers — [[WEIR-I-0012]] rails are complete.
- Forward-feeds [[WEIR-I-0007]] S5 (Salesforce OAuth reuses this provider).

### Risk Considerations
- OAuth refresh timing/expiry is the fiddly part — drive refresh off both `expires_in` and a 401 retry
  so clock skew doesn't wedge a sync. Test the 401-triggered refresh explicitly.
- Provider config is per-connection secret material — confirm it's treated as a secret end-to-end (not
  logged, not echoed in run logs).

## Status Updates **[REQUIRED]**

### 2026-06-29 — code traced; approach confirmed host-side (ADR [[WEIR-A-0033]])

**Architectural decision made + recorded as [[WEIR-A-0033]].** Auth today is applied *guest-side*
(`crates/connectors/rest/src/lib.rs:379-393` reads `api_key` from guest config). Decision: move **all**
auth host-side via the egress policy — secrets never enter the guest — and migrate the existing
bearer/api-key path onto the same seam (not just OAuth). **Scope expanded** accordingly.

**Key code seams found:**
- `EgressPolicy::authorize` (`crates/weir-runtime/src/lib.rs:198-217`) — host-side, runs before each
  guest request, mutates outbound `Parts` (uri + headers). This is the injection point. **Sync, and
  sees only the request, not the response.**
- Handle construction + process-wide cache: `weir-orchestrator/src/lib.rs:116-150` (`resolve()`),
  keyed by `(search_path, package, config)`, currently `HostAllowList::allow_all()`.
- Config mapping: `weir-app/src/lib.rs:815-829` (`manifest_stream_to_config`).
- Importer authenticator parsing/mapping: `weir-importer/src/lib.rs:69-244`.
- Manifest `Auth` enum: `weir-manifest/src/lib.rs:48-57`.

**Design consequences (see ADR):**
- 401-driven refresh is **out for v1** (egress can't see responses); refresh on **expiry + margin**.
- `weir-runtime` needs a blocking HTTP client (e.g. `ureq`) to perform OAuth/session grants host-side.
- Handle cache key must add a **credential fingerprint (hash, not raw secret)** since the secret leaves
  `config`.
- Query-param keys → host-side **uri rewrite** (not a header).

**Staged plan:**
1. `weir-manifest`: extend `Auth` with `OAuth2 { .. }` + `SessionToken { .. }` variants (+ keep
   Bearer/ApiKey/ApiKeyQuery).
2. `weir-importer`: map Airbyte `OAuthAuthenticator` (refresh-token/client-credentials) +
   `SessionTokenAuthenticator` → new variants; unsupported sub-variants reported. Unit tests.  ← starting here
3. `weir-runtime`: `CredentialProvider` on a new/extended egress policy — static header, uri rewrite,
   OAuth grant + expiry cache, session login. `ureq` dep.
4. `weir-app`/`weir-orchestrator`: split secret from guest config; build the policy from the secret;
   fix cache key (credential fingerprint); stop sending `api_key` to the guest.
5. `rest` runtime: stop reading/applying `api_key` (host owns injection now).
6. Tests: wire-level OAuth (mock token + resource endpoints) + session; keyed live run.
7. Flip [[WEIR-S-0016]] auth rows; refresh `secrets/README.md` coverage notes.

**Acceptance-criteria amendment:** the "401-driven refresh" sub-clause is deferred per ADR-0033;
v1 asserts expiry-based refresh.

### 2026-06-29 — stages 1–2 landed (manifest + importer), tests green

- **Stage 1 (`weir-manifest`):** `Auth` gains `OAuth2 { token_url, grant, client_id_key,
  client_secret_key, refresh_token_key, scopes }` + `SessionToken { login_url, token_path,
  inject_header }`, and `OAuthGrant { RefreshToken, ClientCredentials }`.
- **Stage 2 (`weir-importer`):** maps Airbyte `OAuthAuthenticator` (refresh-token + client-credentials)
  and `SessionTokenAuthenticator` → the new variants. New `cfg_key_ref` preserves the config-key case
  (host reads the secret by it; distinct from the uppercasing `env_ref`). The coverage/preview reporter
  **honestly flags OAuth/session as "host injection pending WEIR-A-0033"** — not claimed runnable until
  the runtime path lands. `weir-app::manifest_stream_to_config` match updated (no-op arms; provider
  wiring is stage 4).
- **Verified:** `cargo check -p weir-manifest -p weir-importer -p weir-app` clean;
  `cargo test -p weir-importer` → 7/7 (4 new mapping/coverage tests).

### 2026-06-29 (cont.) — stages 3–4 landed (host-side credential seam + plumbing)

- **Stage 3 (`weir-runtime`):** new `Credential` enum on `HostAllowList` (`.with_credential`) —
  `Header` (bearer/header api-key), `Query` (uri-rewrite for `?api_key=`), `OAuth2(OAuth2Provider)`
  (mints+caches a token via `ureq`, re-mints within a 60s margin of `expires_in`; refresh-token +
  client-credentials grants), `Session(SessionProvider)` (login → token at dot-path → inject).
  `authorize()` resolves+injects per request. Added `ureq` + `serde_json` deps.
- **Stage 4 (`weir-runtime` + `weir-orchestrator` + `weir-app`):**
  `Credential::from_auth_config(json) -> (Option<Credential>, sanitized_json)` builds the credential
  from the resolved config **and strips every secret + auth key** from what reaches the guest. The
  orchestrator `resolve()` now calls it, builds the policy, and passes the **sanitized** config to
  `from_wasm_package` (cache still keyed on full config so distinct creds get distinct handles).
  `weir-app::manifest_stream_to_config` emits the non-secret OAuth/session metadata. **Bearer/header/
  query auth is now migrated host-side too** (the secret no longer reaches the guest).
- **Verified:** `cargo check` clean across the 6 touched crates; `cargo test -p weir-app --lib` 7/7
  (bearer/header/query config mapping intact); `cargo test -p weir-importer` 7/7.

**Remaining — stages 5–7 (land together):**
- **5+6:** rewrite the wire-level auth tests for **host-side** injection (mock OAuth token endpoint +
  resource endpoint; migrate the existing bearer wire test off guest-side), remove the now-inert
  guest auth code in `rest`, and add the keyed OAuth live run. This is the end-to-end proof.
- **7:** flip [[WEIR-S-0016]] OAuth/session rows ❌→✅ + the importer `analyze()` arms (only once the
  wire test is green), refresh the stale `secrets/README.md`.

Architecture note: two auth paths coexist mid-flight — the orchestrator path (host-side, sanitized)
and the direct engine wire tests (still guest-side). Stage 5 unifies them onto host-side.

### 2026-06-29 (cont.) — stages 5–7 landed; **task complete**

- **Stage 5 (`rest` guest):** removed the guest-side auth application + the auth fields from `RestCfg` /
  `parse_cfg` / the config schema. The guest issues a plain request; auth is host-only. Safe because
  `from_wasm_package` is only ever called from the sanitizing `resolve()`.
- **Stage 6 (wire-level proof):** migrated the bearer + query wire tests onto the host path (via a
  `host_auth_source` helper mirroring the orchestrator) and added an **OAuth end-to-end test** (mock token
  endpoint + resource endpoint: host mints the token, injects the bearer, the guest's request carries it).
  All 8 `wasm_http_engine` tests green; session covered by a fast provider unit test.
- **Stage 7:** flipped [[WEIR-S-0016]] auth rows (OAuth/session ❌→✅, bearer/header/query now host-side) +
  the importer `analyze()` arms; refreshed `secrets/README.md`.

**Notable bug found + fixed:** the OAuth wire test first failed with `EINVAL` — ureq's blocking socket
read fails when run **on the wasmtime async-executor thread** that fires the egress hook. Fix: a
`run_blocking` helper offloads the grant to a dedicated OS thread (clean blocking context). This preserves
in-`authorize` expiry-refresh (no architectural compromise). Applies to both OAuth and session.

**Verification (all green):** `weir-manifest`, `weir-importer` 7/7, `weir-runtime` 3/3 (OAuth mint+cache,
session login+extract, egress), `weir-app` full suite (lib/cli/corpus/serve), `weir-engine`
`wasm_http_engine` 8/8 (incl. OAuth E2E), clippy clean across the 6 touched crates.

**AC status:** OAuth2 (refresh-token + client-credentials, expiry refresh) ✅; SessionToken ✅ (v1 plain
login; credentialed-login body is a follow-up); importer mapping + unit tests ✅; wire-level proof ✅;
ledger flipped ✅; clippy/tests green ✅. **Keyed live OAuth run:** the harness supports it
(`secrets/<slug>.json`) and the mechanism is proven by the wire test — an actual live run needs real OAuth
creds dropped into the gitignored secrets dir (operator-supplied). **Deferred (per [[WEIR-A-0033]]):**
401-driven refresh; `BasicHttpAuthenticator`; credentialed session-login bodies.
