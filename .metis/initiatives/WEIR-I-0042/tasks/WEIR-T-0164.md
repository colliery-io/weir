---
id: bootstrap-admin-key-on-fresh-db
level: task
title: "Bootstrap admin key on fresh-DB `weir api` (kill the demo lockout)"
short_code: "WEIR-T-0164"
created_at: 2026-08-16T15:24:00.282038+00:00
updated_at: 2026-08-18T02:14:33.936947+00:00
parent: WEIR-I-0042
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0042
---

# Bootstrap admin key on fresh-DB `weir api` (kill the demo lockout)

## Parent Initiative

[[WEIR-I-0042]]

## Objective **[REQUIRED]**

`weir api` — the container entrypoint — on a fresh store serves an API nobody can authenticate to: the bootstrap admin key is minted only by `weir init`, so the docker demo boots to an unpassable sign-in gate. Mint the bootstrap key idempotently when `weir api`/`weir serve` starts against a fresh store and print it once, so the very first run works.

## Evidence (2026-08-16 alpha review)

- `Dockerfile:46` — ENTRYPOINT runs only `weir api`.
- `crates/weir-cli/src/main.rs:250-256` — `bootstrap_admin_key` is called only under `Init`; the `Api` arm (449-453) does not.
- `crates/weir-app/src/auth.rs:318-323` — `bootstrap_admin_key` is already idempotent.
- `crates/weir-api/src/lib.rs:307-340` — `require_auth` has no open-when-no-keys bypass.
- `weir-ui/src/main.rs:1303-1324` — the full-screen sign-in gate is the first thing a user sees.

## Acceptance Criteria **[REQUIRED]**

- [x] Fresh store + `weir api` (and `weir serve`): admin key minted and printed exactly once with a save-this-now notice; restarts neither re-mint nor re-print
- [x] Cold-start docker demo works: up → sign in with the printed key → UI usable, no repo clone required *(verified 2026-08-25 under [[WEIR-T-0163]]: fresh compose stack minted + printed the key in the weir service logs; the key authenticated `/auth/me` as admin; UI served. "No repo clone" lands when [[WEIR-T-0165]]'s image publish first runs on a tag)*
- [x] Docs (installation + secure-control-plane guide) describe the behavior and the `weir init` pre-mint path
- [x] CLI/serve test covers fresh-start mint + idempotent restart

## Implementation Notes

Mint server-side at startup (weir-app serve path or weir-api::serve) rather than in each CLI arm so every entrypoint benefits. Print only when the key was newly created — never log an existing key on later starts. `bootstrap_admin_key` being idempotent means the fresh-vs-restart distinction is its return value's job.

## Status Updates **[REQUIRED]**

**2026-08-18 — implemented, tested, docs updated (ralph run).**

- `crates/weir-cli/src/main.rs`: new `announce_bootstrap_key(&App)` helper — calls the already-idempotent `App::bootstrap_admin_key()` and prints the plaintext banner only when a key was newly minted; wired into both the `Serve` and `Api` arms right after `App::open` (the `Api` arm is the Docker entrypoint). `Init` unchanged; `Runner` deliberately not wired (worker pods shouldn't mint).
- New integration test `crates/weir-cli/tests/bootstrap.rs` (`serve_mints_bootstrap_key_once_then_never_again`): spawns the real `weir` binary on a fresh temp store, asserts the `weirk_` line appears; restarts on the same store, asserts the serving banner is reached with no key line. Passes in ~2s. `tempfile` added as weir-cli dev-dependency.
- Docs: `docs/guides/secure-control-plane.md` leads with mint-on-first-serve, keeps `weir init` as the pre-mint path; `docs/reference/installation.md` gains a "First run" section.
- `angreal check all` green (one `cargo fmt` pass applied). Work left uncommitted for end-of-ralph review.
