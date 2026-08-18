---
id: readme-rewrite-what-weir-is-the
level: task
title: "README rewrite — what weir is + the real quickstart"
short_code: "WEIR-T-0163"
created_at: 2026-08-16T15:23:58.851367+00:00
updated_at: 2026-08-16T15:23:58.851367+00:00
parent: WEIR-I-0042
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/todo"


exit_criteria_met: false
initiative_id: WEIR-I-0042
---

# README rewrite — what weir is + the real quickstart

## Parent Initiative

[[WEIR-I-0042]]

## Objective **[REQUIRED]**

Replace the stub README.md with a real front door: what weir is, and the shortest working path to a running pipeline. Today README.md says "An early-stage Rust project" and documents a nonexistent `crates/weir-core` layout, with no pointer to the docs or any install path — every visitor's first impression is wrong.

## Evidence (2026-08-16 alpha review)

- `README.md` — stub; references nonexistent `weir-core` (verified).
- The real fastest path (`docker compose --profile demo up --build`) exists only as a comment inside `compose.yml`.
- `docs/tutorials/first-sync.md` was verified line-by-line accurate against the current CLI — the README just needs to route to it.
- `mkdocs.yml` site_description also says "An early-stage Rust project".

## Acceptance Criteria **[REQUIRED]**

- [ ] README states what weir is (open-source no-code ingestion + reverse-ETL platform; Rust control plane, WASM connectors, embedded web UI; Apache-2.0, ASF-bound) and describes the actual workspace layout
- [ ] Quickstart section with the docker compose demo path and links to `docs/tutorials/first-sync.md` and the docs site
- [ ] No references to nonexistent crates; `mkdocs.yml` site_description updated to match
- [ ] License / contributing pointers appropriate for an Apache-2.0 project
- [ ] Quickstart depends on [[WEIR-T-0164]] (bootstrap key) and [[WEIR-T-0173]] (slim demo profile) landing for its steps to be literally true — coordinate wording or land after

## Implementation Notes

Pure docs/markdown work; no code changes. Keep the README short — the docs site carries the depth. Verify the quickstart commands by running them, not by pattern-matching the compose file.

## Status Updates **[REQUIRED]**

*To be added during implementation*
