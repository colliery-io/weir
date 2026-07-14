---
id: connector-onboarding-pipeline
level: initiative
title: "Connector onboarding pipeline (manifest → shared runtime → catalog → connection)"
short_code: "WEIR-I-0012"
created_at: 2026-06-24T19:49:03.624218+00:00
updated_at: 2026-06-28T01:37:11.081812+00:00
parent: WEIR-V-0001
blocked_by: []
archived: true

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: M
initiative_id: connector-onboarding-pipeline
---

# Connector onboarding pipeline (manifest → shared runtime → catalog → connection) Initiative

## Context **[REQUIRED]**

[[WEIR-A-0032]] decided how connectors are distributed + run: low-code connectors are **`manifest.yaml`
(data) executed by a shared declarative runtime** (no compile); full-code connectors are per-connector Rust
crates (compiled at onboard). [[WEIR-S-0015]] specs the **onboarding interfaces** that make this real.

This initiative **delivers those rails**: the end-to-end path that turns a connector definition into a
running, named catalog connector. It is a **prerequisite for [[WEIR-I-0008]]** (Airbyte declarative parity):
parity work *widens what travels on the rails* (the declarative surface the runtime/importer cover), but a
translated manifest with nowhere to land + run is just a passing importer test. The rails must exist first.

Today the ingress is **crate-centric** (`Source::LocalCrate` / `Folder`); there is no way to register a
*manifest-as-data* connector, the shared runtime (`rest`) isn't wired as a manifest host, and there's no
preview gate or manifest UI. (See the mismatch table in [[WEIR-S-0015]].)

## Goals & Non-Goals **[REQUIRED]**

**Goals:**
- A **manifest** can be onboarded into the catalog as a **named connector** that runs on the **shared
  declarative runtime** — **no compile, instant** ([[WEIR-A-0032]] interpreter model).
- The full-code crate path (compile at onboard) is reconciled alongside it; onboarding is **manual** with the
  three phase-1 gestures from [[WEIR-S-0015]] (discover & select / point-me-to-it / base-runtime + YAML).
- A pre-commit **preview** (tier / confidence / unsupported) gate — never onboard a silently-broken connector
  ([[WEIR-A-0020]]).
- An onboarded connector composes into connections (existing dropdowns) + runs end-to-end.

**Non-Goals:**
- Deepening the runtime/importer's declarative **coverage** (auth/pagination/routers/transforms) — that is
  [[WEIR-I-0008]]; this initiative uses the runtime's *current* capability + surfaces gaps via preview.
- A hosted **manifest registry / hub** + automated sync (phase 1 vendors manifests in-repo).
- Codegen-to-crate (rejected, [[WEIR-A-0032]]); import-as-a-job (deferred — manifests are instant, crates
  show a spinner).

## Detailed Design **[REQUIRED]**

Implements [[WEIR-S-0015]]. Slices:

**S1 — Backend rails (manifest → runtime → catalog → resolution).** `Source::Manifest { yaml, name }` in
`weir-app` ingress; `weir-importer` maps the manifest → the shared runtime's config; catalog entries gain a
**kind** (`Wasm{package}` | `Manifest{yaml}`); a `Manifest` connector **resolves** to the shared
declarative-runtime package + the bound manifest/config (per-connection config layers on top). No codegen, no
compile. *Done = register a manifest + a connection on it runs end-to-end (generalizes today's `rick-live`).*

**S2 — Preview gate.** `weir-importer` emits a fidelity report (tier / confidence / streams / unsupported);
`POST /catalog/preview { manifest }` returns it synchronously (no run). Onboarding surfaces it before commit.

**S3 — Onboarding API + UI.** Extend `POST /catalog/import` with `manifest` (instant register); widen
`GET /catalog/available` to list vendored manifests + crates; a dedicated UI **"Onboard a connector"** view
with the three gestures, manifest editor/upload + preview, instant register (spinner only for the crate
path), visually separate from the connection form.

**S4 — Vendored manifest corpus.** A curated in-repo `manifests/` set (authored from official API docs) surfaced
in "discover & select" — the phase-1 stand-in for a registry.

## Alternatives Considered **[REQUIRED]**

- **Fold onboarding into [[WEIR-I-0008]].** Rejected — onboarding is the *rails*, parity is *coverage on the
  rails*; onboarding is also broader than Airbyte (carries full-code crates + the demo). Conflating them
  hides the dependency.
- **Skip named manifest connectors; keep ad-hoc per-connection config** (today's `rick-live`). Rejected —
  doesn't give a reusable, named, previewable connector; the operator re-types the manifest config per
  connection.
- Codegen / prebuilt-hub / interpreter trade-offs are settled in [[WEIR-A-0032]].

## Implementation Plan **[REQUIRED]**

S1 (rails) is the prerequisite spine — land it first; it unblocks [[WEIR-I-0008]]'s *runnable* coverage and
the demo's "real" connectors. S2 (preview) + S3 (API/UI) make it safe + usable; S4 (corpus) is content.
Each slice ends green (workspace + clippy). Decomposed into tasks below.
