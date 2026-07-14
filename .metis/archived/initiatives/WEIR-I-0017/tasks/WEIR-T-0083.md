---
id: auth-schema-principal-api-keys
level: task
title: "Auth schema + Principal + API keys (hash/verify, CLI mint, bootstrap)"
short_code: "WEIR-T-0083"
created_at: 2026-07-05T21:09:56.914643+00:00
updated_at: 2026-07-05T21:24:53.379266+00:00
parent: WEIR-I-0017
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0017
---

# Auth schema + Principal + API keys (hash/verify, CLI mint, bootstrap)

## Parent Initiative

[[WEIR-I-0017]] (load-bearing). The data + key primitives every later slice builds on. Governed by [[WEIR-A-0008]].

## Objective

Add the control-plane auth schema, the `Principal` type, API-key hash/verify, the `weir auth token create`
CLI, and a bootstrap admin key minted at `init` — **no middleware yet** (that's [[WEIR-T-0084]]). At the end,
keys can be minted + verified in isolation; the API is still open.

## Reference

- **Schema:** `angreal schema gen` regenerates per-backend migrations + `schema.rs` from the **logical DDL**
  (diesel-dualdb; see `.angreal/task_schema.py` + the existing `crates/*/…/schema` DDL sources). Add tables
  there, then `angreal schema gen` — do **not** hand-edit generated `schema.rs`/migrations.
- **CLI:** `crates/weir-cli` — existing subcommands (`init`, `api`, …); add an `auth` subcommand group.
- **App:** `crates/weir-app` owns the control-plane store; the `Principal` + key store live near it.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [ ] **Schema (logical DDL → `angreal schema gen`):**
  - `api_keys` — `id`, `name`, `hash` (argon2), `prefix` (last 8 chars of the key, indexed, for O(1)
    lookup so verify isn't a full-table argon2 scan), `created_at`, `last_used_at` (nullable), `revoked_at`
    (nullable).
  - `sessions` — `id` (opaque), `subject`, `created_at`, `expires_at`, `csrf_token` (used in [[WEIR-T-0086]]).
  - `audit_events` — `id`, `actor`, `action`, `resource`, `ts`, `outcome` (used in [[WEIR-T-0085]]).
  - Regenerated for **both** backends (sqlite + postgres); `angreal schema gen` clean.
- [ ] **`Principal`** type — `Principal { subject: String, kind: PrincipalKind (Key | User), … }`.
- [ ] **Key primitives** — generate a high-entropy key (`weirk_<random>`), argon2-hash it, store
  `(name, hash, prefix)`; `verify(presented) -> Option<Principal>` (prefix lookup → argon2 verify →
  stamp `last_used_at`; skip revoked). Use a vetted crate (`argon2`).
- [ ] **CLI** — `weir auth token create --name <n>` mints a key, stores it, prints the **plaintext once**
  (never again). `weir auth token list` (name/prefix/created/last-used, never the secret). `weir auth token
  revoke <prefix|name>`.
- [ ] **Bootstrap** — `weir … init` mints an initial admin key **if none exist** and prints it once with a
  clear "save this" notice (idempotent: no new key if any exist).
- [ ] Unit tests: mint→verify roundtrip, wrong key rejected, revoked rejected, prefix lookup. clippy clean;
  no attribution trailers.

## Technical Notes

- Key format `weirk_<base62(32 bytes)>`; `prefix` = last 8 chars (public, for lookup). Argon2 default params.
- Keep it storage-agnostic through the existing diesel-dualdb store layer (works on sqlite + postgres).
- This task deliberately does **not** touch the router — the API stays open until [[WEIR-T-0084]].

## Dependencies

- Prereq for [[WEIR-T-0084]]/[[WEIR-T-0085]]/[[WEIR-T-0086]].

## Status Updates

### 2026-07-05 — schema + key primitives + CLI done

- **Migration** `0002_auth` (logical DDL) → `angreal schema gen` → `api_keys` (id/name/hash/prefix/
  created_at/last_used_at/revoked_at + `idx_api_keys_prefix`), `sessions` (id/subject/created_at/expires_at/
  csrf_token), `audit_events` (id/actor/action/resource/ts/outcome + ts index). Regenerated both backends.
- **Multi-migration runner** — `weir-schema::migrate` was hardcoded to `0001_init` only; refactored to an
  ordered `MIGRATIONS` list applied once each (tracked in `__weir_schema_version`). Existing DBs pick up
  `0002_auth` on next open; fresh DBs get both. `version_applied(conn, version)` now parameterized.
- **`weir-app/src/auth.rs`** — `Principal { subject, kind: Key|User }`; key gen (`weirk_<40 alphanumeric>`,
  last-8 `prefix`), argon2 hash/verify; App methods `create_api_key` (returns plaintext once),
  `verify_api_key` (prefix lookup → argon2 → stamp `last_used_at`, skip revoked), `list_api_keys`,
  `revoke_api_key`, `bootstrap_admin_key` (mint `admin` iff none exist). Deps: diesel-dualdb/uuid/argon2/rand.
- **CLI** — `weir auth token create/list/revoke`; `init` mints + prints the bootstrap admin key (idempotent).

**Verified:** 4 unit tests (mint→verify, wrong-key, revoked, bootstrap-idempotent) pass; CLI smoke
(init→admin key, create/list/revoke, revoked state shown) works; clippy clean. The API is still open —
middleware is [[WEIR-T-0084]]. **Gotcha:** the test `TempDir` guard must outlive the `App` or sqlite goes
readonly (deleted dir). **Complete.**
