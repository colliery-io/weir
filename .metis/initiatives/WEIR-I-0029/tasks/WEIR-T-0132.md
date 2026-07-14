---
id: per-side-config-surface-test-docs
level: task
title: "Per-side config surface + test + docs"
short_code: "WEIR-T-0132"
created_at: 2026-07-08T11:06:27.394948+00:00
updated_at: 2026-07-08T11:31:07.243214+00:00
parent: WEIR-I-0029
blocked_by: [WEIR-T-0131]
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0029
---

# Per-side config surface + test + docs

## Parent Initiative

[[WEIR-I-0029]] — make it settable, prove it, correct the docs. Closes the initiative.

## Objective

Expose per-side config on the CLI + API (with a `config` convenience that sets both), prove source + dest receive
different config, and remove the shared-config caveat from the docs.

## Reference

- The split from [[WEIR-T-0131]].
- `crates/weir-cli/src/main.rs` — `ConnAction::Add`; `crates/weir-api/src/lib.rs` — `ConnectionDto` +
  `into_connection`/`from_connection`.
- `docs/guides/cdc-deletes.md` (the "same-connector source + destination" note) + `docs/reference/connection-config.md`.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [x] CLI `connection add` gains `--source-config` / `--dest-config` (each overrides `--config` for its side);
  API `ConnectionDto` carries `source_config`/`dest_config` with `config` as the both-sides convenience.
- [x] `per_side_config_round_trips` proves the two sides persist + reload **different** config (pg→pg `orders`
  vs `orders_replica`); `work_spec_splits` (T-0131) proves the wiring.
- [x] Docs: the "same-connector loop" caveat is gone — `guides/cdc-deletes.md` shows a real pg→pg CDC connection
  with `--source-config`/`--dest-config`; `reference/connection-config.md` + `cli.md` document the per-side
  fields. `mkdocs build --strict` clean.
- [x] Full weir-app + orchestrator suites + clippy clean.

## Status Updates

### 2026-07-08 — done (`d2480be`), closes I-0029

Per-side config is real for operators + the docs reflect it. CLI `--source-config`/`--dest-config` override the
`--config` convenience; the round-trip test proves the two configs persist independently; the caveat is replaced
with a working pg→pg example. Verified: full weir-app suite + orchestrator suite + clippy + mkdocs `--strict`
all clean. The split is proven at the unit level (`work_spec_splits`) + persistence level
(`per_side_config_round_trips`), the executor resolves each side with its own config, and the delete mechanics
are already proven live by T-0117. **Complete — closes [[WEIR-I-0029]].**
