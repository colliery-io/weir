---
id: documentation-diátaxis-pass
level: initiative
title: "Documentation — Diátaxis pass"
short_code: "WEIR-I-0027"
created_at: 2026-07-08T02:03:50.118213+00:00
updated_at: 2026-07-08T03:00:13.483460+00:00
parent: WEIR-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: M
initiative_id: documentation-diátaxis-pass
---

# Documentation — Diátaxis pass

## Context

The docs are stubs. `docs/index.md` still calls weir "an early-stage Rust project"; `getting-started` is 20
lines; `api/index.md` points at a generator. The only substantive pages are three good how-to guides
(field-mapping, connector-authoring, soak-testing). Meanwhile the platform has grown ~10 initiatives —
auth/OIDC + API keys, multi-tenancy, connections + scheduling, CDC + delete propagation, typed schemas +
evolution, holistic ops/health views, WASM connector model, k8s deployment — **none of it documented**.

Reorganise the docs around **[Diátaxis](https://diataxis.fr)** — the four user needs, kept distinct:
**Tutorials** (learning, hands-on), **How-to guides** (task, steps), **Reference** (information, dry + accurate),
**Explanation** (understanding, why). Source material already exists in the specs (WEIR-S-0001..0016) + ADRs.

## Goals & Non-Goals

**Goals:**
- A nav restructured into the four Diátaxis modes, each page in the right mode (no how-to/reference bleed).
- Cover the real platform: the core user journey end-to-end + the major capabilities added since.
- Keep the established voice of the three existing guides (specific, contract-precise, example-led).

**Non-Goals:**
- Rewriting the auto-generated API reference internals (keep the plissken-rendered `api/` as the API ref;
  wrap it with a hand-written orientation page).
- New ADRs / design changes — this is documentation of what exists, verified against current code.
- Marketing/site-chrome work beyond the mkdocs nav.

## Proposed structure (Diátaxis)

**Tutorials** (learning-oriented, one guided path):
- *Your first sync* — install → start the server → onboard a connector → create a connection → run → see records.

**How-to guides** (task-oriented; existing ✓ + new):
- Map fields ✓ · Author a connector ✓ · Soak-test ✓
- Onboard a declarative (manifest) connector · Schedule syncs · Capture changes + propagate deletes (CDC) ·
  Secure the control plane (OIDC + API keys) · Manage tenants · Deploy to Kubernetes.

**Reference** (information-oriented, accurate):
- CLI (`weir` commands) · HTTP API (orientation + the generated ref) · Connector contract (the WIT trait,
  `RecordBatch`/`ChangeOp`) · Connection config (sync modes, write modes, mapping ops, typed `FieldType`).

**Explanation** (understanding-oriented, why):
- Architecture overview (control plane · engine · orchestrator · WASM runtime · UI) · The connector + WASM
  sandbox + secrets-off-platform model · Multi-tenancy · Delivery guarantees (checkpoints/outbox/at-least-once)
  · Typed schemas + evolution · Deployment topology.

### Design decisions (2026-07-08, approved)

- **Depth: comprehensive** — all four modes fully populated (the page inventory above).
- **Nav: full four-mode rename** — top-level nav becomes **Tutorials / How-to guides / Reference / Explanation**;
  "Getting Started" folds into Tutorials (+ install → Reference), the three guides move under How-to guides.
- **Workflow: per-mode tasks** — one task per Diátaxis mode, ralphed with commits + `mkdocs build` checks.
- **Reference**: hand-write the CLI + config + contract reference (derived from `--help` / the WIT / the code,
  verified); keep the plissken-generated API ref, add a hand-written orientation page around it.
- Documentation only — verified against current code; no new ADRs.

## Proposed decomposition (for sign-off)

- **T-a — Nav reorg + Tutorial:** restructure `mkdocs.yml` into the four modes; rewrite `index.md` (what weir
  is); write the *Your first sync* tutorial (install → server → onboard → connect → run → see records); move the
  three existing guides under How-to. `mkdocs build` clean.
- **T-b — How-to guides:** add task-oriented guides — onboard a manifest connector, schedule syncs, CDC + delete
  propagation, secure the control plane (OIDC + API keys), manage tenants, deploy to Kubernetes.
- **T-c — Reference:** CLI (`weir` commands), HTTP API orientation (+ generated ref), the connector contract
  (WIT trait, `RecordBatch`/`ChangeOp`), connection config (sync/write modes, mapping ops, `FieldType`).
- **T-d — Explanation:** architecture overview, the WASM + secrets-off-platform model, multi-tenancy, delivery
  guarantees, typed schemas + evolution, deployment topology. Closes the initiative.

## Exit Criteria (draft)

- [ ] Nav reflects the four Diátaxis modes; every page sits in exactly one.
- [ ] The core journey + the major post-stub capabilities are documented, verified against current code.
- [ ] `mkdocs build` is clean (no broken links/nav); the three existing guides are retained/placed.
