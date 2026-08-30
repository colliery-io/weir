---
id: alpha-quick-wins-front-door-honest
level: initiative
title: "Alpha quick wins — front door, honest feedback, packaging hygiene"
short_code: "WEIR-I-0042"
created_at: 2026-08-16T15:22:27.476190+00:00
updated_at: 2026-08-30T11:43:10.018976+00:00
parent: WEIR-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: M
initiative_id: alpha-quick-wins-front-door-honest
---

# Alpha quick wins — front door, honest feedback, packaging hygiene Initiative

## Context **[REQUIRED]**

The 2026-08-16 alpha-readiness review (10-agent survey + assessment over the full tree at commit 1905ab8; build health green — `angreal check all`, `test unit` 118 passed, `test manifests` corpus all tier A) found the core loop genuinely alpha-grade but the outside-user first hour broken. Both independent assessors converged: the blockers are packaging, onboarding, and feedback-surfacing — not architecture — and a cluster of hours-to-a-day fixes removes the worst walls.

This initiative collects exactly those quick wins. Each child task carries the file:line evidence from the review so it is executable without rediscovery. The larger alpha workstreams (real-source reach: DB TLS, OAuth PKCE, UI sync/write-mode surface; durability: mid-read checkpoints, work_units retention; multi-tenant hardening; secrets at rest) are deliberately excluded and will be scoped as their own initiatives.

## Goals & Non-Goals **[REQUIRED]**

**Goals:**
- An outsider can find, install, and authenticate into weir without reading source: honest README, published container image, release artifacts that actually contain the UI + connectors, bootstrap key on first run.
- Misconfiguration fails early and loudly: connector existence/config validated at creation time, server error bodies visible in the UI.
- Continuous operation survives ordinary turbulence: retries on by default, clean ctrl-c with live residents, stranded-resident recovery, schedule picks up config edits.
- Packaging/drift hygiene closed: s3 staged, mssql/snowflake/resident in the contract drift guard, docs match reality, demo compose profile slimmed.

**Non-Goals:**
- DB TLS (postgres/mssql), OAuth authorization-code flow, UI sync/write-mode + per-side config forms, mid-read checkpointing/streaming reads, multi-tenant hardening (cross-tenant stop hole, OIDC mapping, tenant-delete cascade), secrets-at-rest/redact-on-read, live validation of the 31 keyed manifests — all real, all tracked separately as the main alpha push.

## Detailed Design **[REQUIRED]**

One child task per fix; every task embeds its evidence (file:line) from the review. Task list is recorded in the Implementation Plan below once decomposed. No shared design work is needed — each fix is local and independently mergeable.

## Alternatives Considered **[REQUIRED]**

- **Fold these into the three larger alpha initiatives (front door / honest feedback / real-source reach).** Rejected: each quick win is independently landable in hours, and landing them first unblocks every later validation loop (demo estate, live suite, outside testers). The larger initiatives stay cleanly scoped to the work that actually needs design.
- **Fix only the demo path and defer the rest.** Rejected: the review showed the failure-feedback holes (stub check(), 201-then-fail, UI error swallowing) hit maintainers too — they tax every debugging session, not just outsiders.

## Implementation Plan **[REQUIRED]**

All tasks are independent and mergeable in any order; the only soft ordering is that the release-workflow fix should land before an image-publish tag is cut. Suggested execution: `/metis-ralph-initiative WEIR-I-0042` or task-by-task.

Task list (decomposed 2026-08-16):

- [[WEIR-T-0163]] — README rewrite: what weir is + the real quickstart
- [[WEIR-T-0164]] — Bootstrap admin key on fresh-DB `weir api` (kill the demo lockout)
- [[WEIR-T-0165]] — Release artifacts that work: trunk + staged connectors in tarballs, ghcr.io image publish
- [[WEIR-T-0166]] — Validate connector existence + config shape at POST /connections (4xx, not 201-then-fail)
- [[WEIR-T-0167]] — UI error surfacing: stop swallowing HTTP failures, show server reasons in toasts
- [[WEIR-T-0168]] — Flagship manifest fidelity: pagination (+incremental) for stripe/hubspot
- [[WEIR-T-0169]] — Turn retries on in production: WorkerConfig knobs, default max_attempts=3
- [[WEIR-T-0170]] — Clean shutdown + stranded-resident recovery (stop_all on ctrl-c, reclaim at startup)
- [[WEIR-T-0171]] — Schedule fidelity: re-register on config change; schedule all tenants
- [[WEIR-T-0172]] — Connector packaging hygiene: drift guard covers all guests, stage s3
- [[WEIR-T-0173]] — Slim the demo compose profile: weir+Postgres only, test deps behind their own profile
- [[WEIR-T-0174]] — Doc staleness sweep: installation list, manifests README, failure triage page (land late — sibling tasks change its ground truth)

Soft orderings only: [[WEIR-T-0165]] before the next v* tag; [[WEIR-T-0163]] quickstart wording depends on [[WEIR-T-0164]]/[[WEIR-T-0173]]; [[WEIR-T-0174]] last.

## Exit Criteria

- [ ] All child tasks completed
- [ ] Cold-start proof: `docker run` of the published image → sign in with the printed bootstrap key → create a no-auth pipeline (e.g. frankfurter → arrow-sink) in the UI → rows land — without cloning the repo
- [ ] A typo'd connector name or malformed config is rejected at creation time, and the server's reason is visible in the UI toast
- [ ] `weir serve` with a live resident source exits cleanly on ctrl-c; a transient run failure retries without operator action
