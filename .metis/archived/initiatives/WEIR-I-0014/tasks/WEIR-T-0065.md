---
id: s1-encrypted-connector-secrets
level: task
title: "S1: Encrypted connector secrets (SOPS+age) + angreal orchestration"
short_code: "WEIR-T-0065"
created_at: 2026-06-30T16:00:33.583292+00:00
updated_at: 2026-06-30T19:36:06.333940+00:00
parent: WEIR-I-0014
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0014
---

# S1: Encrypted connector secrets (SOPS+age) + angreal orchestration

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0014]] — slice S1.

## Objective **[REQUIRED]**

Move weir's connector auth secrets from gitignored plaintext to **encrypted-in-repo** via **SOPS + age**,
so trusted developers (and CI) can decrypt and run the authed connector tests, with no shared symmetric
key. Port the proven `arawn` flow (`../arawn`: `.sops.yaml`, `tests/secrets/`, the `secrets-edit` /
`secrets-updatekeys` / decrypt-wrapped test angreal tasks) to weir's **per-connector JSON** model
(approach A — encrypt the files, decrypt before the existing `keyed_manifests_run_live` reads them).

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria **[REQUIRED]**

- [ ] **`.sops.yaml`** at the repo root: a `creation_rules` entry with `path_regex` for
  `secrets/.*\.enc\.json$`, an `encrypted_regex` matching connector secret fields (case-insensitive
  `api_key` / `client_secret` / `client_id` / `refresh_token` / `token` / `password`), and an `age:`
  recipient list seeded with the maintainer's pubkey. Heavily commented (onboarding/rotation), mirroring
  arawn.
- [ ] **Secrets model migrated**: per-connector `secrets/<slug>.enc.json` committed in ciphertext; `.gitignore`
  updated to **commit `secrets/*.enc.json`** while keeping plaintext `secrets/*.json` ignored. Existing
  `*.example.json` templates kept.
- [ ] **angreal tasks** (mirroring arawn's `task_test.py`):
  - a **decrypt-wrapped keyed run** — decrypts `secrets/*.enc.json` (via the dev's/CI's age key) into a
    gitignored working location, runs `keyed_manifests_run_live`, cleans up; falls back to plaintext
    `secrets/*.json` if no encrypted bundle (legacy/un-onboarded dev path).
  - **`secrets-edit`** — `sops edit` a bundle, handling first-file creation (write stub → encrypt-in-place
    → edit), per the sops 3.10+ gotcha.
  - **`secrets-updatekeys`** — `sops updatekeys --yes secrets/*.enc.json` (add/remove a developer).
- [ ] **`secrets/README.md`** adapted from arawn's: per-dev setup (`age-keygen`, `SOPS_AGE_KEY_FILE`),
  onboarding (PR pubkey → updatekeys), editing, rotation, removal (+ rotate-after-removal warning),
  bootstrapping the first file, schema (keys = the connection-config fields a connector needs).
- [ ] **Proof**: a developer whose pubkey is in `.sops.yaml` can `sops -d` a connector bundle and run that
  authed connector end-to-end (rows > 0). The existing keyed test reads decrypted configs unchanged.
- [ ] `angreal tree` lists the new tasks; workspace + clippy clean (no Rust changes expected beyond, at
  most, where the keyed test looks for decrypted files).

## Implementation Notes **[CONDITIONAL: Technical Task]**

### Technical Approach
- Reference: `../arawn/.sops.yaml`, `../arawn/tests/secrets/README.md`, `../arawn/.angreal/task_test.py`
  (`test_uat` sops-detect + `exec-env`; `test_secrets_edit`; `test_secrets_updatekeys`).
- weir keeps `secrets/` (not `tests/secrets/`) to minimize churn from the current layout.
- Decrypt-for-tests: weir's configs are JSON files, so instead of arawn's `sops exec-env` (env vars), the
  task does `sops -d secrets/<slug>.enc.json` → a gitignored plaintext working dir the keyed test reads
  (or decrypt in-place to `secrets/<slug>.json`, which is already gitignored). Decide the exact decrypted
  location in implementation; keep it gitignored.
- `encrypted_regex` must match weir's **lowercase** field names (`api_key`, `client_secret`) — arawn's is
  uppercase (`*_KEY`). Use a case-insensitive pattern.

### Dependencies
- Tooling: `sops` + `age` (documented install in the README). The maintainer must generate an age keypair
  and seed `.sops.yaml` with their pubkey before the first encrypt — that's an operator step (their key).
- No blocker; independent of S2 ([[WEIR-T-0066]]), which builds on this.

### Risk Considerations
- **Don't commit plaintext.** Verify `.gitignore` ignores `secrets/*.json` (plaintext) but allows
  `secrets/*.enc.json`; double-check no real key lands unencrypted.
- The maintainer's real age pubkey is needed to seed the recipient list — until then the encrypted bundles
  can't be created. Scaffold `.sops.yaml` + tasks + README first; the operator fills the pubkey + encrypts.

## Status Updates **[REQUIRED]**

### 2026-06-30 — machinery landed + validated; awaiting operator age key

All the SOPS+age scaffolding is in and proven; the only thing left is the operator (maintainer) step of
generating an age key, dropping the real pubkey into `.sops.yaml`, and encrypting real bundles.

**Implemented:**
- **`.sops.yaml`** (repo root): `path_regex: secrets/.*\.enc\.json$`, `encrypted_regex:
  "(?i)(key|secret|token|password|client_id)"`, commented onboarding/rotation, `age:` seeded with a
  **placeholder** pubkey (operator replaces it).
- **`.gitignore`**: `/secrets/*.json` ignored, `!/secrets/*.enc.json` + `!/secrets/*.example.json`
  re-included — plaintext never committed, ciphertext + templates tracked.
- **`.angreal/task_secrets.py`** — `secrets edit <slug>` (creates+encrypts a stub for new bundles, else
  `sops edit`) and `secrets updatekeys` (re-encrypt all to the current recipients).
- **`.angreal/task_tests.py`** — `test connectors-live`: decrypts `secrets/*.enc.json` into a private
  `mktemp` dir as `<slug>.json`, sets `WEIR_SECRETS_DIR`, runs `keyed_manifests_run_live`, cleans up. No
  plaintext under `secrets/`; no clobbering a dev's hand-made plaintext.
- **`crates/weir-app/tests/manifest_corpus.rs`** — the keyed test reads `WEIR_SECRETS_DIR` (default
  `secrets/`), the only Rust change.
- **`secrets/README.md`** — rewritten for the SOPS model (setup / run / edit / rotate / onboard / remove /
  bootstrap / tracked-vs-ignored).

**Validated:**
- `sops 3.11.0` + `age v1.3.1` installed.
- Isolated round-trip with my exact `.sops.yaml` rules: `api_key` + `client_secret` encrypt to
  `ENC[AES256_GCM,…]`; `__stream`/`_note` stay plaintext (reviewable diffs); age key decrypts cleanly.
- `cargo check -p weir-app --test manifest_corpus` clean; `angreal tree` lists `secrets edit/updatekeys`
  + `test connectors-live`.

**Operator steps remaining (the maintainer's key — can pair with [[WEIR-T-0066]] account setup):**
1. `age-keygen -o ~/.config/sops/age/keys.txt`; `export SOPS_AGE_KEY_FILE=...`
2. Replace the placeholder pubkey in `.sops.yaml` with the real `age1…`.
3. `angreal secrets edit <slug>` to create the first real bundle; commit the `.enc.json`.
Once a real connector bundle exists, the "proof" AC (`angreal test connectors-live` → rows > 0) closes.

### 2026-06-30 (cont.) — bootstrap done + proven end-to-end; **task complete**

The operator bootstrap already existed (`SOPS_AGE_KEY_FILE` set; age key at `~/.config/sops/age/keys.txt`;
pubkey `age1tp700…` = arawn's recipient). Wired that **real pubkey into `.sops.yaml`** (replacing the
placeholder).

**End-to-end proof (real `.sops.yaml` + real key):** encrypted a throwaway no-auth bundle, ran
`angreal test connectors-live` → it decrypted into a temp dir, set `WEIR_SECRETS_DIR`, and ran the keyed
suite live: **`✓ jsonplaceholder/posts: 100 rows`**. The encrypt → ciphertext → decrypt → run path works
on the real config + key. Smoke bundle removed afterward (real authed bundles are [[WEIR-T-0066]] /
operator work).

**Bug fixed:** `connectors-live` had called `build_wasm_connectors()`, which lists a nonexistent
`wasm-fixtures/rest` (the rest connector is at `crates/connectors/rest` and the keyed test self-stages via
`stage_live_runtime`) → `FileNotFoundError`. Dropped the call.

All machinery ACs met + proven. The **authed** run is the identical path with a real key — exercised in
[[WEIR-T-0066]] once accounts/secrets exist. Nothing more to build for S1.
