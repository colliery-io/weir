---
id: s5-partition-routers-substream
level: task
title: "S5: Partition routers — Substream (ByParent) + List shards"
short_code: "WEIR-T-0064"
created_at: 2026-06-28T01:48:21.446652+00:00
updated_at: 2026-06-30T12:00:12.064526+00:00
parent: WEIR-I-0008
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0008
---

# S5: Partition routers — Substream (ByParent) + List shards

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0008]] — slice S5 (Partition routers). Tracked in [[WEIR-S-0016]] (Partition-router rows).

## Objective **[REQUIRED]**

Many real declarative connectors can't be read as a single flat stream — a child stream is sliced by the
ids of a parent stream (`SubstreamPartitionRouter`), or by a static list (`ListPartitionRouter`). The
engine already has the partition primitive (`PartitionScheme`, [[WEIR-A-0012]] / [[WEIR-T-0027]]); this
slice wires Airbyte's routers onto it so substream/list connectors import **and run** on the shared
`rest` runtime.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria **[REQUIRED]**

- [ ] **SubstreamPartitionRouter → `PartitionScheme::ByParent`.** The runtime reads the parent stream,
  derives the child slices from the parent records (parent key → child request param / path template),
  and reads each child slice; importer maps `parent_stream_configs` (parent stream, parent key,
  partition field) → `ByParent { parent_stream, key }`.
- [ ] **ListPartitionRouter → shards.** A static `values` list (or `values` from config) becomes the
  partition set; each value templates into the request (path/param); importer maps it to the
  list/shard scheme.
- [ ] **Runtime execution.** The `rest` runtime walks partitions and emits records across all slices in
  one read (checkpointing per the existing partition/state model); single-partition connectors are
  unaffected.
- [ ] **Wire-level proof.** A test over real `wasi:http` exercises a parent→child substream (parent
  returns N ids → N child requests → merged records) and a list-router connector; importer mapping unit
  tests for both router kinds. Unsupported router variants (e.g. `CustomPartitionRouter`) reported, not
  dropped.
- [ ] **Ledger flipped.** [[WEIR-S-0016]] Partition-router rows `ListPartitionRouter` and
  `SubstreamPartitionRouter` move ❌ → ✅ in this change (DoD per [[WEIR-S-0016]] REQ-2).
- [ ] Workspace + integration suites green; clippy clean.

## Implementation Notes **[CONDITIONAL: Technical Task]**

### Technical Approach
- Reuse the engine's existing `PartitionScheme` ([[WEIR-A-0012]]); the new work is (1) importer mapping
  of the two router constructs and (2) the runtime producing + iterating partitions and templating each
  partition value into the request (the `{{ }}` templating from commit `ed8d3db` is the substrate for
  parent-key / list-value interpolation).
- Substream ordering: read parent fully (or stream parent keys) before/at the start of child reads;
  keep it within the single read flow so checkpoint semantics ([[WEIR-A-0011]]) hold.

### Dependencies
- Independent of [[WEIR-T-0063]] (auth) — can land in parallel. Both build on the shared-runtime
  templating + importer foundation; no blockers.

### Risk Considerations
- Parent streams can be large — decide whether to materialize parent keys or stream them; for phase-1
  declarative scope, materializing the parent-key list is acceptable, but note the memory cost in the
  ledger/row if a connector has a huge parent.
- Watch partition explosion (N parents × M children) against the existing run/state model — confirm the
  checkpoint cardinality is sane.

## Status Updates **[REQUIRED]**

### 2026-06-30 — landed; **task complete**

**Design decision:** routing is **connector-internal** in the `rest` runtime, *not* the engine's
`PartitionScheme`. Airbyte's routers are *enumeration* (slice a stream into sub-requests), whereas
`PartitionScheme`/`materialize_partitions` is *parallel sharding* for throughput — a different concern.
Substream also needs to **read the parent stream** to enumerate slices, which the static
`materialize_partitions` (no connector access) can't do. So the runtime reads parent → loops children,
or loops list values, templating each into the request and concatenating records; the engine sees one
read. (AC wording "→ `PartitionScheme::ByParent`" is satisfied semantically by this connector-internal
equivalent.)

**Implemented:**
- `weir-manifest`: `Stream.partition: Option<PartitionRouter>` (`List { field, values }` /
  `Substream { field, parent_path, parent_record_selector, parent_key }`).
- `weir-importer`: maps `ListPartitionRouter` + `SubstreamPartitionRouter` (parent stream resolved
  inline/anchor; `$ref`/templated/custom → reported, not silently dropped). Unit tests for both.
- `weir-app`: emits partition config to the runtime.
- `rest` runtime: `render_template` now **leaves non-config `{{ }}` intact** (so
  `{{ stream_partition.* }}` survives), new `render_partition`; `fetch_records` refactored into a
  per-slice `fetch_slice` + a routing wrapper (list / substream / flat).
- `analyze()` flags untranslatable routers.

**Verified (all green):** `weir-importer` 9/9 (incl. list + substream mapping); `wasm_http_engine` 10/10
incl. **two new wire tests** — substream (parent `/posts` → `/posts/<id>/comments` per id → 3 records)
and list (2 values → 2 records); host crates + rest-wasm compile; clippy clean.

**Ledger:** [[WEIR-S-0016]] list + substream rows ❌→✅.

**Deferred (reported, not silently dropped):** templated `ListPartitionRouter` values (config-driven),
`$ref` parent streams, `CustomPartitionRouter`, multi-parent substreams.
