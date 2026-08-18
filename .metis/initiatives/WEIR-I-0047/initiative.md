---
id: user-facing-secrets-story
level: initiative
title: "User-facing secrets story — redaction, write-only fields, references"
short_code: "WEIR-I-0047"
created_at: 2026-08-18T01:57:44.476636+00:00
updated_at: 2026-08-18T02:04:34.985582+00:00
parent: WEIR-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/design"


exit_criteria_met: false
estimated_complexity: S
initiative_id: user-facing-secrets-story
---

# User-facing secrets story — redaction, write-only fields, references Initiative

## Context **[REQUIRED]**

The implementation contradicts the project's own decided secrets ADRs. [[WEIR-A-0037]] requires "re-read the secret per run; never cache across rotation"; [[WEIR-A-0013]] requires a plaintext-free control plane. Today (2026-08-16 review, verified): connection credentials are stored plaintext in the connections table and **echoed verbatim to any Read-level key** via ConnectionDto (`crates/weir-api/src/lib.rs:131-153`); secret sanitization is an allowlist of known auth schemes, so an unrecognized `auth_scheme` passes secrets into the guest unchanged (`weir-runtime` Credential::from_auth_config fallthrough); HANDLE_CACHE keys embed secret-bearing config JSON and are never evicted (`crates/weir-orchestrator/src/lib.rs:147,197`); MINTED_TOKENS never invalidate on rotation. Host-side injection ([[WEIR-A-0033]]) protects the WASM guest — not the store, backups, or the API.

The good news: the plumbing for the fix exists. The connector `config_schema` already carries a per-field `secret` flag (the UI renders password inputs from it — verified `weir-ui/src/main.rs:984`), so the server can redact and merge on the same marking. [[WEIR-A-0021]] was closed against A-0037 on 2026-08-17: there is no backend to abstract — env/file consumption is the interface.

## Goals & Non-Goals **[REQUIRED]**

**Goals:**
- **Redact-on-read:** GET /connections never returns secret-flagged values; they render as a fixed sentinel.
- **Write-only merge:** POST /connections upsert preserves the stored secret when a field carries the sentinel or is omitted — which unblocks the UI edit flow ([[WEIR-I-0045]] task 4).
- **Secret references:** `env:NAME` / `file:/path` values resolved **host-side at credential-build time, per run** — the A-0037-pure path (weir reads from where the operator's secret system puts it; rotation is transparent).
- **Fail-closed sanitization:** an unknown `auth_scheme` is rejected at creation (or all non-allowlisted keys stripped) — never passed through to the guest.
- **Cache hygiene:** HANDLE_CACHE keys become hashes (no plaintext secret material as map keys); MINTED_TOKENS invalidate on config change.

**Non-Goals:**
- At-rest encryption of config columns — the alpha posture is documented (protect the DB/filesystem; references avoid storing secrets at all); revisit post-alpha.
- Any secret store, rotation scheduler, or Vault/KMS integration — excluded by [[WEIR-A-0037]].
- OAuth token acquisition — [[WEIR-I-0043]] owns the flow; this initiative owns how tokens are *stored* (the same write-only/reference machinery).

## Detailed Design **[REQUIRED]**

**Open design questions:**

1. **What marks a field secret, server-side.** The connector's `config_schema` secret flag is the natural single source; the wrinkle is fields the *manifest baking* adds (auth_* keys layered by `manifest_stream_to_config`) that have no schema entry. Recommendation: schema flag ∪ a small server-side pattern list for the auth_* vocabulary (until the [[WEIR-I-0033]] typed refactor gives it one home).
2. **The sentinel.** A fixed string (e.g. `__weir_secret_unchanged__`) vs null-means-preserve. Recommendation: fixed sentinel — null/omitted is ambiguous with "clear this field". One convention shared with [[WEIR-I-0045]]'s edit form; decide once, here.
3. **Reference syntax & scope.** `env:NAME` and `file:/path` resolved in the host at credential build (`Credential::from_auth_config` seam, `crates/weir-orchestrator/src/lib.rs:157`), re-read per run per [[WEIR-A-0037]]; never resolvable by Read-level API calls. Question: allowed for non-secret fields too, or secret-flagged only? Recommendation: secret-flagged only (keeps template semantics simple).
4. **Migration.** Existing rows hold literals; redaction changes GET behavior for them immediately (safe), write-only merge is backward-compatible. No data migration needed; document the behavior change in the CHANGELOG per [[WEIR-A-0006]].

### Candidate decomposition

| # | Task | Effort | Notes |
|---|---|---|---|
| 1 | Redact-on-read + write-only sentinel merge (API + store), keyed off the secret marking; API tests both ways | week | unblocks [[WEIR-I-0045]] task 4 |
| 2 | `env:`/`file:` reference resolution host-side at credential build, per run | days | the A-0037-pure path |
| 3 | Fail-closed sanitization for unknown auth schemes (reject at creation, pairs with [[WEIR-T-0166]]) | days | closes the allowlist fallthrough |
| 4 | Cache hygiene: HANDLE_CACHE hashed keys; MINTED_TOKENS invalidation on config change | days | coordinates with [[WEIR-I-0044]] task 7 (size bounding) |
| 5 | At-rest posture doc + threat-model note (what redaction does and does not protect) | days | honest docs |

## Alternatives Considered **[REQUIRED]**

- **At-rest encryption of config columns now** — deferred: real work (key management questions that A-0037 deliberately avoids), while references remove the stored secret entirely for operators who care; redaction + posture doc covers the alpha.
- **A secrets backend trait** — rejected by the [[WEIR-A-0021]] closure (superseded by [[WEIR-A-0037]]).
- **Literals-only with redaction (no references)** — rejected: references are days of work and are the only mode that satisfies A-0037's rotation-transparency requirement end-to-end.

## Implementation Plan **[REQUIRED]**

Task 1 first (it unblocks the UI edit flow and is the biggest exposure); 2-4 in any order after; 5 with the docs pass. **This initiative is sequenced first among the post-quick-wins initiatives** — it is small (~2 weeks total), and [[WEIR-I-0045]] (edit flow) and [[WEIR-I-0043]] (OAuth token storage) both depend on its conventions.

Dependencies: none inbound. Outbound consumers: [[WEIR-I-0045]] task 4, [[WEIR-I-0043]] task 5, [[WEIR-I-0044]] task 7 (cache), [[WEIR-T-0166]] (rejection surface for task 3).

## Exit Criteria

- [ ] No API response contains a secret-flagged value (API test sweeps every connection-returning route)
- [ ] Editing a connection without retyping secrets works end-to-end (with [[WEIR-I-0045]])
- [ ] A credential supplied as `env:`/`file:` reference syncs, and rotating the underlying value takes effect on the next run without touching weir
- [ ] An unknown auth_scheme is rejected at creation with a clear message; no config path passes unsanitized secrets to a guest
