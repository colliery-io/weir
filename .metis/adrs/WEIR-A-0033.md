---
id: 001-host-side-credential-injection
level: adr
title: "Host-side credential injection — secrets never enter the guest"
number: 1
short_code: "WEIR-A-0033"
created_at: 2026-06-29T12:28:14.038468+00:00
updated_at: 2026-06-29T12:28:14.038468+00:00
decision_date: 2026-06-29
decision_maker: dylan.storey
parent:
archived: false

tags:
  - "#adr"
  - "#phase/decided"


exit_criteria_met: true
initiative_id: NULL
---

# ADR-33: Host-side credential injection — secrets never enter the guest

## Context **[REQUIRED]**

weir's core security claim ([[WEIR-A-0002]] isolation, [[WEIR-V-0001]]) is that it can run
**untrusted, community-contributed connectors safely**. Connectors run as WASM guests with
capability-gated egress; a REST connector is granted `http`.

Today, authentication is applied **guest-side**: the shared `rest` runtime reads
`auth_scheme`/`auth_name`/`api_key` from its per-connection config and sets the
`Authorization` header (or `?api_key=`) itself (`crates/connectors/rest/src/lib.rs:379-393`).
The secret (`api_key`) is passed **into the guest config**. [[WEIR-A-0013]] (secret resolution
path) anticipated host-side redemption but deferred it ("the real secret-handle redemption
lands later", `crates/weir-manifest/src/lib.rs:44`).

This is a real vulnerability, not a stylistic one: **a guest with `http` egress that also holds
the secret can exfiltrate it** — POST your credential to an attacker. The sandbox is meaningless
for credential confidentiality while the credential lives inside the sandboxed code. Adding
**OAuth2** (WEIR-T-0063) sharpens this to a point: a *refresh token* is a long-lived,
high-value credential and the worst possible secret to place in untrusted code.

The host already has the seam to do this correctly: `EgressPolicy::authorize`
(`crates/weir-runtime/src/lib.rs:198-217`) runs host-side **before** each guest request and can
mutate the outbound `Parts` (uri + headers) — it already injects `inject_headers`. It is just
unused by the current auth path.

## Decision **[REQUIRED]**

**All connector authentication is injected host-side by the egress policy. Secrets never enter
the guest config.** Concretely:

1. A host-side **credential provider** carried by the egress policy resolves the auth scheme to
   a concrete injection at `authorize` time:
   - **Bearer / header api-key** → inject the `Authorization` (or named) header.
   - **Query-param api-key** → rewrite the outbound `uri` to append `?<name>=<key>`.
   - **OAuth2** (refresh-token + client-credentials grants) → the host performs the token
     `POST` itself, caches the access token with its `expires_in`, re-mints when expired (a
     safety margin before expiry), and injects `Authorization: Bearer <token>`.
   - **Session token** → host performs the login request, extracts the token (path-configurable),
     injects it on subsequent requests.
2. The **guest never receives** `api_key`, `client_secret`, `refresh_token`, or session
   credentials. weir-app/orchestrator extract the secret material from the connection config and
   route it to the policy; only non-secret config reaches the guest.
3. The **existing bearer/api-key path migrates onto this same seam** — we do not ship a
   half-secure model where api-keys stay guest-side while OAuth is host-side. One seam, all
   schemes.

This concretizes the deferred portion of [[WEIR-A-0013]].

## Alternatives Analysis **[CONDITIONAL: Complex Decision]**

| Option | Pros | Cons | Risk Level | Implementation Cost |
|--------|------|------|------------|-------------------|
| **Host-side injection, all schemes (chosen)** | Secrets never in untrusted code; sandbox claim becomes true; fixes existing api-key hole; builds the seam Salesforce/reverse-ETL reuse | Larger change across runtime/app/importer; host needs an HTTP client for grants; handle cache must key on credential identity | Medium | L |
| OAuth guest-side (mirror current api-key path) | Smallest change; consistent with today | Puts refresh tokens in untrusted code — *deepens* the vulnerability with the worst secret type | High (security) | S |
| OAuth host-side, leave api-key guest-side | Smaller than full migration | Two auth code paths; ships a half-secure model; api-key hole remains | Medium | M |

## Rationale **[REQUIRED]**

The chosen option is the only one consistent with weir's reason to exist. If "run untrusted
connectors safely" is the pitch, credential confidentiality against the connector itself is not
optional — and it is exactly what a competitor's security review would probe. Doing it at the
moment we add OAuth means we (a) never introduce refresh tokens into the guest, (b) close the
pre-existing api-key exfiltration hole as a byproduct, and (c) build the host-side credential
seam once, which the most security-sensitive future work (Salesforce destination, [[WEIR-I-0007]])
reuses directly. The extra cost buys the truth of the central claim.

## Consequences **[REQUIRED]**

### Positive
- The sandbox's confidentiality guarantee becomes real: a malicious connector cannot read or
  exfiltrate the credential — it never holds it.
- A materially stronger security narrative: "your credentials never touch connector code, even
  for connectors you didn't write."
- The OAuth-refresh seam is reused by Salesforce/reverse-ETL ([[WEIR-I-0007]] S5).

### Negative
- Larger, multi-crate change (runtime egress policy, weir-app/orchestrator plumbing, `rest`
  runtime, importer).
- `weir-runtime` gains a blocking HTTP client dependency (e.g. `ureq`) to perform OAuth/session
  grants host-side.
- The process-wide handle cache (keyed by `(search_path, package, config)`,
  `crates/weir-orchestrator/src/lib.rs:116`) must incorporate a **credential fingerprint (hash,
  never the raw secret)** now that the secret no longer rides in `config` — else two connections
  to the same API with different keys would share a handle.

### Neutral
- **401-driven token refresh is out of scope at the egress layer**: `authorize` sees only the
  outbound request, not the response. v1 refreshes on **expiry** (with a safety margin), which
  covers the common case; 401-triggered refresh is a follow-up requiring deeper wasi:http
  response plumbing.
- Query-param api-keys are handled by host-side **uri rewrite** rather than header injection —
  same seam, slightly different mechanism.

## Review Schedule **[CONDITIONAL: Temporary Decision]**

### Review Triggers
- A connector family needs **401-driven** refresh (not just expiry-based) — revisit egress
  response visibility.
- The secret-handle store from [[WEIR-A-0013]] lands — the provider should redeem handles rather
  than hold raw secrets in policy memory.
