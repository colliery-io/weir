---
id: declarative-connector-integration
level: initiative
title: "Declarative connector integration testing (SOPS-encrypted secrets + live suite)"
short_code: "WEIR-I-0014"
created_at: 2026-06-30T15:59:30.734252+00:00
updated_at: 2026-08-30T11:43:11.699672+00:00
parent: WEIR-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: M
initiative_id: declarative-connector-integration
---

# Declarative connector integration testing (SOPS-encrypted secrets + live suite) Initiative

## Context **[REQUIRED]**

The declarative connector surface now covers a lot (auth incl. OAuth/session via [[WEIR-A-0033]],
pagination, partition routers — see the [[WEIR-S-0016]] ledger). But our only authed test is
`keyed_manifests_run_live` (`crates/weir-app/tests/manifest_corpus.rs`), which reads **gitignored**
`secrets/<slug>.json` files. That means: (a) auth tokens can't be shared with trusted developers or used
in CI — each dev re-sources keys by hand, and CI can't run the authed connectors at all; and (b) there's
no real **integration suite** that demonstrates the connectors actually work end-to-end and flags
breaking changes (ours *or* upstream API drift).

A sibling repo (`arawn`) already runs a proven flow: **SOPS + age**, per-developer keypairs, encrypted
secrets committed in ciphertext, and angreal tasks that orchestrate decrypt-for-tests / edit / add-a-dev.
This initiative ports that flow to weir's per-connector model and builds the live suite on top of it.

## Goals & Non-Goals **[REQUIRED]**

**Goals:**
- **Encrypted-in-repo connector secrets** (SOPS + age): per-connector `secrets/<slug>.enc.json` committed
  in ciphertext, decryptable by any developer whose age public key is in `.sops.yaml`. No shared symmetric
  key; onboarding = "PR your pubkey, a recipient runs `sops updatekeys`."
- **angreal orchestration** mirroring arawn: a decrypt-wrapped keyed test run, `secrets-edit`, and
  `secrets-updatekeys` (add/remove a developer), plus an adapted `secrets/README.md`.
- **A live integration suite** that, per authed connector, asserts **functional invariants** (discover →
  expected streams; sync → rows > 0 + expected key fields present) so it demonstrates functionality and
  fails loudly on a breaking change — without brittle exact-data snapshots.
- **CI wiring**: a **nightly + on-demand** job that decrypts with an age key from a CI secret and runs the
  suite, so breaking changes surface daily without flaky live APIs blocking PRs.

**Non-Goals:**
- A coverage/"fidelity" harness or a "parity %" number — coverage tracking is the [[WEIR-S-0016]] ledger's
  job (see the [[WEIR-T-0053]] closure). This initiative is *functional* testing, not breadth measurement.
- Per-PR live tests (flaky; blocks merges) — live runs are nightly + manual.
- Exact-record snapshot assertions (live data drifts → false failures).
- Encrypting non-test/production secrets, or a runtime secret store ([[WEIR-A-0013]] is separate).

## Architecture **[CONDITIONAL: Technically Complex Initiative]**

### Overview
- **`.sops.yaml`** at the repo root: a `creation_rules` entry with `path_regex` for `secrets/.*\.enc\.json`,
  an `encrypted_regex` matching connector secret fields (api_key / client_secret / refresh_token / token /
  password — case-insensitive), and an `age:` recipient list (one `age1…` pubkey per developer). Nothing
  in this file is sensitive (age pubkeys are public, like SSH pubkeys).
- **Secrets** live at `secrets/<slug>.enc.json`, committed in ciphertext (SOPS encrypts the matched values
  only, so structure + diffs stay readable). Plaintext `secrets/<slug>.json` stays gitignored as a working
  form.
- **Per-developer age keypair** (`~/.config/sops/age/keys.txt`, `SOPS_AGE_KEY_FILE`). Onboarding/rotation/
  removal exactly as arawn (PR pubkey → `sops updatekeys` → commit ciphertext).
- **Decrypt for tests**: an angreal task decrypts the per-connector bundles (`sops -d`) into a gitignored
  working location the existing `keyed_manifests_run_live` reads, runs the suite, cleans up. (weir's
  per-connector JSON model is kept — approach A; arawn's flat `sops exec-env` env-var model was the
  alternative, rejected below.)

### Sequence (a trusted dev / CI running the suite)
`angreal test connectors-live` → decrypt `secrets/*.enc.json` (via the dev's / CI's age key) → run the
keyed integration suite over the decrypted per-connector configs → per-connector functional assertions →
clean up plaintext.

## Detailed Design **[REQUIRED]**

Decomposed into two tasks:

- **S1 — Encrypted secrets (SOPS + age) + angreal orchestration.** `.sops.yaml`; migrate `secrets/` to the
  encrypted-in-repo model; angreal tasks (decrypt-wrapped keyed run / `secrets-edit` / `secrets-updatekeys`);
  gitignore updates (commit `*.enc.json`, keep plaintext `*.json` ignored); adapt `secrets/README.md` from
  arawn's. Done = a trusted dev with their key in `.sops.yaml` can decrypt and run an authed connector.
- **S2 — Live integration suite + CI.** Turn the keyed runs into per-connector **functional assertions**
  (discover → streams; sync → rows>0 + expected fields), covering no-auth + authed connectors; wire a CI
  job (nightly schedule + `workflow_dispatch`) that decrypts with `SOPS_AGE_KEY` from CI secrets and runs
  it. Done = the suite is green locally + in CI and a deliberately-broken connector turns it red.

## Alternatives Considered **[REQUIRED]**

- **Flat env-var bundle + `sops exec-env`** (arawn's exact model). Rejected as the primary: weir's
  per-connector configs are multi-field (OAuth = client_id/secret/refresh_token), which a flat env bundle
  models awkwardly and would force a rewrite of the keyed test. Approach A (encrypt the per-connector JSON,
  decrypt before the test) keeps the existing model.
- **git-crypt** (transparent file encryption). Rejected: opaque binary blobs in git (unreadable diffs), and
  SOPS's value-level encryption + age key management is cleaner for a multi-dev recipient list.
- **Per-PR live integration tests.** Rejected: live APIs are flaky (rate limits, data drift); a red live
  test must never block an unrelated PR. Nightly + on-demand gives the signal without the blockage.
- **Exact-record snapshots.** Rejected: live data changes → false failures. Functional invariants catch
  real breakage (a construct stops working / an API changes shape) without the noise.

## Implementation Plan **[REQUIRED]**

S1 (secrets + angreal) first — it's the prerequisite for any authed run, and stands alone (a dev can
decrypt + run the existing keyed test). S2 (assertions + CI) builds the demonstrate-functionality /
catch-breaking-changes layer on top. Each ends green (workspace + clippy); S2 ends with CI green + a
proven red-on-break. Decomposed into tasks below.

## Scope decision (2026-08-30)

Dylan will not provision further connector accounts ([[WEIR-T-0067]] archived). The initiative's live
surface is therefore: the **no-auth tier** (5 connectors, live-verified — see `manifests/verified.json`,
[[WEIR-T-0183]]) plus the **three existing bundles** (`openweather`, `github`, `nasa` — together covering
the header / bearer / query-param injection paths). The machinery (SOPS + angreal + suite +
`connectors-live.yml`) is complete and user-consumable; the remaining optional step is CI enablement
(`SOPS_AGE_KEY` repo secret) for the nightly run. Read the exit criteria below against that scope.

## Exit Criteria

- [ ] `.sops.yaml` + encrypted `secrets/*.enc.json` committed; plaintext stays gitignored; README adapted.
- [ ] angreal tasks: decrypt-wrapped keyed run, `secrets-edit`, `secrets-updatekeys` — all working.
- [ ] A trusted developer (pubkey in `.sops.yaml`) can decrypt and run an authed connector end-to-end.
- [ ] Live suite asserts per-connector functional invariants (discover + rows>0 + expected fields) for
      no-auth + authed connectors; a deliberately-broken connector turns it red.
- [ ] CI: nightly + on-demand job decrypts with `SOPS_AGE_KEY` and runs the suite; green.
- [ ] Workspace + clippy clean.
