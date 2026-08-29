---
id: doc-staleness-sweep-installation
level: task
title: "Doc staleness sweep — installation list, manifests README, failure triage page"
short_code: "WEIR-T-0174"
created_at: 2026-08-16T15:24:14.306320+00:00
updated_at: 2026-08-29T02:14:12.384642+00:00
parent: WEIR-I-0042
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0042
---

# Doc staleness sweep — installation list, manifests README, failure triage page

## Parent Initiative

[[WEIR-I-0042]]

## Objective **[REQUIRED]**

Sweep the doc-staleness edges the review caught and add the missing first-hour failure-triage page, so the otherwise-accurate docs stay trustworthy. The Diátaxis pass (WEIR-I-0027) was verified against early-July code; everything since (per-side config, demo connectors, resident Start/Stop, new auth schemes) post-dates it.

## Evidence (2026-08-16 alpha review)

- `docs/reference/installation.md` — staged-connector list says echo/slow/arrow-sink/postgres/rest/rest-dest; the script actually stages snowflake/mssql/faulty too (and s3 after [[WEIR-T-0172]]).
- `manifests/README.md` — says 34 connectors (dir has 36), omits google-analytics + google-sheets, and names 5 manifests absent from the repo entirely (restcountries, youtube, usgs, weatherapi, dogceo).
- `docs/reference/cli.md` — omits the resident start/stop commands.
- No failure-triage doc exists: empty WEIR_CONNECTORS_DIR, unknown-connector, and first-run auth lockout live only in code/task comments.
- `docs/guides/demo-pipelines.md` — deep-links into `.metis/` paths that the in-progress archival churn will break.

## Acceptance Criteria **[REQUIRED]**

- [x] installation.md's staged-connector list matches `stage-connectors.sh` output (state where the list comes from so the next drift is caught)
- [x] manifests/README.md count + list matches the directory; phantom entries removed
- [x] cli.md covers the full clap surface including start/stop
- [x] New how-to: "first-hour triage" — connectors dir empty, unknown connector, auth lockout (→ weir init / bootstrap key per [[WEIR-T-0164]]), port conflicts, and where run errors / logs / dead-letters live
- [x] `.metis` deep links in public docs replaced with stable targets or removed

## Implementation Notes

Docs-only task; verify claims against the current code (clap surface, staging script) rather than the review text — several sibling tasks in this initiative will have changed the ground truth by the time this runs, so land it late in the initiative. Build with `angreal docs build` to catch nav/link errors.

## Status Updates **[REQUIRED]**

**2026-08-28 — completed.** All five edges swept, verified against current code (not the review text):

- **installation.md**: staged-connector list regenerated from `scripts/stage-connectors.sh` (now incl. snowflake, mssql, faulty, s3) with a note that the script is the source of truth; added Docker stacks (demo vs integration profiles), port-override table (`WEIR_HTTP_HOST_PORT`/`WEIR_PG_HOST_PORT`/`WEIR_MSSQL_HOST_PORT`/`WEIR_MINIO_HOST_PORT`), Runtime knobs section (`WEIR_MAX_ATTEMPTS`, `WEIR_RETRY_BASE_MS`), and First run (mint-once bootstrap key).
- **manifests/README.md**: count corrected to 36 and list matches the directory — google-analytics + google-sheets added, the 5 phantom entries (restcountries, youtube, usgs, weatherapi, dogceo) removed; added a Verification-honesty section stating which manifests are corpus-validated vs live-verified.
- **cli.md**: start/stop rows added, `--execution-mode` documented, bootstrap-key behavior noted on serve/api/init.
- **New guide**: `docs/guides/first-hour-triage.md` (+ mkdocs nav) — covers the fresh-install sign-in/bootstrap-key lockout with rescue path, unknown-source-connector (empty/unset `WEIR_CONNECTORS_DIR` as the classic cause), missing-required-config errors, run failures/retries and the control-plane-error banner, Docker port conflicts, stale-schedule expectations, and a where-things-live table (runs, logs, dead-letters, state, health).
- **`.metis` deep links**: removed from `docs/guides/demo-pipelines.md` (WEIR-S-0018 now referenced by tracker ID in prose, no path links).
- **Verification**: site builds clean and strict — `mkdocs build --strict` (via `uvx --from mkdocs-material`, since local PATH has no mkdocs; `angreal docs build` requires it) with only the pre-existing INFO lines about api/weir.md and api/rust/weir-cli.md not being in nav. `angreal check all` clean.
