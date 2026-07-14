---
id: connector-catalog-directory-backed
level: initiative
title: "Connector catalog (directory-backed registry)"
short_code: "WEIR-I-0010"
created_at: 2026-06-22T17:47:49.318017+00:00
updated_at: 2026-06-24T04:45:04.210702+00:00
parent: WEIR-V-0001
blocked_by: []
archived: true

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: L
initiative_id: connector-catalog-directory-backed
---

# Connector catalog (directory-backed registry) Initiative

Realizes the MVP of [[WEIR-S-0007]] (Connector Catalog / Registry).

## Context **[REQUIRED]**

Today connectors are **compiled into the weir binary** (`use weir_connector_echo as _;` …). Fine for a
handful; the **Airbyte parity arc ([[WEIR-I-0008]])** inverts the scale — translating the long tail
produces hundreds of connectors that can't be statically linked. They need a **home + a runtime
registry**: a place to live, be enumerated, and be instantiated on demand.

The substrate already exists: the **package format is decided** ([[WEIR-A-0015]] — a connector is a
wasm-component package: `package.toml` + `.wasm`, signable), and the host already loads packages from a
directory (`ConnectorHandle::from_wasm_package(search_path, …)`). What's missing is the **catalog over
it** — enumeration + discovery + instantiation. This is also the backbone the deferred [[WEIR-I-0009]]
S5 dropdowns + `discover()` stream pickers plug into.

fidius is **purely the plugin contract**, not plugin management — so the registry is **weir's** to build
(user direction, 2026-06-22), not a fidius concern.

## Goals & Non-Goals **[REQUIRED]**

> **Reconciled 2026-06-23** after [[WEIR-I-0011]] (WASM-always) completed: the catalog is **wasm-only**.
> `origin` (**first-party | community | private**) is now an **attribute**, not a packaging *kind* — there
> is no native tier (A-0016 superseded by [[WEIR-A-0030]]; `native_in_process` deleted; `ConnectorRef` is
> wasm-only). The deferred I-0011 follow-up ("`ConnectorRef::Wasm` gains `version`/`origin`") folds into S1.
>
> **Publication contract now defined ([[WEIR-A-0018]] v1, 2026-06-23):** three ingress paths —
> curated first-party allowlist, open community crates.io crawl (view-only), and **direct import**
> (crates.io `(name,version)` | git | local path) — all through one *fetch → compile → snapshot spec →
> register* pipeline; `[package.metadata.weir]` carries `contract`+`roles` for pre-compile listing. The
> I-0010 **folder-scan MVP is the degenerate local-import case**; crates.io crawl + import are the
> post-MVP "sync." This sharpens S2's registration as the generic ingress pipeline.

**Goals:**
- A **weir-owned, persisted catalog** (a table in the resilient diesel-dualdb `Store`) of registered
  **wasm connector packages**, each with a snapshotted spec (roles, `config_schema`, version) + an
  `origin` attribute (first-party | community). Managed (register/unregister), durable across restarts,
  enumerable by a cheap table read. ("Directory-backed" = the `connectors/` dir holds the wasm
  *artifacts*; the catalog *index* lives in the store.)
- A **`connectors/` directory convention** for wasm artifacts; register → snapshot spec → instantiate via
  `from_wasm_package`. The parity arc registers its output here.
- A **`/connectors` enumeration API** + **UI dropdowns** (source/dest + stream discovery) — closing the
  deferred I-0009 S5.
- The catalog is the **target** parity ([[WEIR-I-0008]]) emits connectors into.

**Non-Goals:**
- **Artifact-storage backend** (separate `connectors` repo, OCI registry, an "artifactory") — that is the
  open decision **[[WEIR-A-0018]]** (still `draft`). Local filesystem directory **only** for this
  initiative; escalate later when catalog scale + contributor community demand it.
- **crates.io availability discovery + pin→pull→compile→register** (the long-run "native list" sourced by
  scanning crates.io for `weir-connector-*` crates/versions). The MVP discovers availability by **folder
  scan** only; the registration seam is *designed to accept* the crates.io path later ([[WEIR-A-0018]]),
  but building it is out of scope here.
- Signature **verification/trust enforcement** (A-0015 covers the format; enforcement is later).
- **Fan-out / multi-destination** (one source → many dests, pipeline graph) — connections stay **1 source
  → 1 destination** (Airbyte-parity); a separate future initiative if wanted (user direction, 2026-06-23).

**Open (deliberately unresolved):**
- **Transforms as catalog-able, versioned artifacts** — today in-flight `MappingSpec` is inline connection
  config (Airbyte would say transforms are dbt's job). Whether mappings ever become first-class catalog
  entries is left open (user did not rule it out); kept out of the MVP build, revisit before it grows up.

## Detailed Design **[REQUIRED]**

The catalog is a **persisted, managed entity** (user direction, 2026-06-22) — a `connectors` table in the
existing **resilient store** (the diesel-dualdb `Store`: SQLite locally, Postgres in prod, so durability/
HA come for free). It sits alongside the connection table; enumeration is a **table read**, not a
load-every-connector scan.

**Three layers, the catalog being the new one:**
- **Catalog (new, persisted)** — one row per registered **wasm package** **`(name, version)`** (semver;
  multiple versions coexist, per [[WEIR-A-0019]]): `location` (package name + path in `connectors/`),
  `roles`, `config_schema`, `contract_range`, `supported_sync_modes`, `origin` (first-party|community),
  `status`, timestamps. Spec **snapshotted at registration** (cheap listing, no load). **PK = (name,
  version); no "active"/latest column** — pinning is per-connection.
- **Connections (exists, persisted)** — one row per configured *instance*; `source`/`dest` are
  `ConnectorRef`s that **pin a catalog `(name, version)`**, frozen at creation ([[WEIR-A-0019]]), plus
  config + schedule. `ConnectorRef::Wasm` **gains `version`** (+ `origin`); existing connection rows
  backfill a version on migration. Dispatch gates the pinned version's `contract_range` against the engine.
- **Load (exists)** — `ConnectorRef::resolve` → `from_wasm_package`. Unchanged (already wasm-only after
  [[WEIR-I-0011]]).

**Registration is the management surface — explicit + compile-gated (user direction, 2026-06-23):**
Registering is a *deliberate act that makes a built wasm package usable* — **not** an automatic startup
scan. The catalog has two faces, kept separate:
- **Registered** (the `connectors` table) — usable, versioned packages; connections pin these.
- **Available** (a discovery scan, *not* the table) — "what can be registered."
  - **MVP — folder scan:** enumerate already-built packages in `connectors/`; register one → load once →
    snapshot spec → `(name, version)` row + artifact pointer. **This is the seam parity
    ([[WEIR-I-0008]]) emits into** (it drops packages in the dir; an operator registers them).
  - **Future ([[WEIR-A-0018]]) — crates.io index:** scan crates.io for `weir-connector-*` crates +
    versions (the long-run "native list"); pin one → **pull + compile** to a wasm package → register.
    Out of scope here; the registration seam is shaped to accept it.

**Surface:** `GET /connectors` (read the catalog: name, kind, roles, config_schema), register/unregister
(manage the catalog), and `GET /connectors/{name}/discover` (streams — kept **live**, since `discover()`
is config-dependent, not snapshotted). The UI turns source/dest into role-filtered dropdowns over the
catalog and feeds the selected entry's `config_schema` into the existing schema-driven form (I-0009 S5).

## Implementation Plan (proposed slices)

- **S1 — Versioned catalog store + per-connection pinning.** `connectors` table in `Store` keyed
  **`(name, version)`** (semver, `roles`, `config_schema`, `contract_range`, `origin`,
  `supported_sync_modes`, `status`, timestamps); **`ConnectorRef::Wasm` gains `version` (+ `origin`)** +
  backfill existing connection rows; registry read API over the table; **dispatch-time contract-range
  gate** (pinned version vs running engine). Unit-tested (persist → reopen → still enumerable; a pinned
  version survives a newer registration). *(Subsumes the deferred I-0011 ConnectorRef follow-up.)*
- **S2 — Folder-scan availability + explicit registration.** `connectors/` dir convention; an
  **availability scan** that enumerates already-built packages in the dir (the "what can I register"
  list); **register** a chosen package **at its version** (load once → snapshot spec → `(name, version)`
  row + artifact pointer, `origin=first-party`); instantiate via `from_wasm_package`. Registration is the
  seam parity ([[WEIR-I-0008]]) emits into. Seed with the rest package + a scan→register→pin→load→run
  conformance test.
- **S3 — Catalog management API + UI dropdowns.** `GET /connectors` (registered) + an
  **available-to-register** listing (folder scan) + register/unregister; source/dest dropdowns
  (role-filtered) over the *registered* catalog feeding the schema-driven config form. Closes deferred
  I-0009 S5 (connector discovery).
- **S4 — Stream discovery in the UI.** `discover()` endpoint + stream dropdown for the selected source
  (kept live — `discover()` is config-dependent, not snapshotted).

Runs **parallel to [[WEIR-I-0008]]** (parity): S1–S2 give parity a durable target to register into; S3–S4
make the catalog manageable + usable from the UI.

## Alternatives Considered

- **Separate `connectors` repo + artifact store / OCI registry now** — cleaner IP/ASF boundary and the
  eventual scaled form, but premature before the catalog has volume or contributors. Kept as the
  escalation path under [[WEIR-A-0018]]; start directory-backed (user direction).
- **Push enumeration upstream into fidius** — rejected: fidius is the plugin *contract*, not plugin
  *management*.
- **Keep static linking only** — doesn't survive the parity long tail.
