# Connector contract

Every connector — declarative or full-code — implements one WASM trait and exchanges data in a small set of
canonical types. The types live in `weir-connector-types`; the trait is defined in each connector's
`wit/connector.wit`.

## The trait

```rust
fn spec(&self) -> ConnectorSpec;                        // name, roles, sync modes, config JSON-schema
fn check(&self, config: Config) -> CheckResult;         // cheap validity probe
fn discover(&self, config: Config) -> DiscoverOutcome;  // the streams this connector exposes
fn read(&self, ctx: ReadContext) -> Stream<ReadMessage>;   // a Source: records + checkpoints
fn write(&self, ctx: WriteContext, batches: Stream<RecordBatch>) -> WriteOutcome;  // a Destination
```

A connector declares its **roles** (`Source`, `Destination`, `ReverseEtl`) and **supported sync modes** in
`spec()`. See [Author a connector](../guides/connector-authoring.md) for the full walk-through.

## Record batches

A read/write moves `RecordBatch`es, in one of three encodings:

| Variant | Shape | Use |
| --- | --- | --- |
| `Rows(Vec<String>)` | one JSON-object row per entry (text) | the common path |
| `Arrow(ArrowIpc)` | Arrow IPC stream bytes | bulk/native |
| `Changes(Vec<ChangeRecord>)` | rows tagged with an op | CDC |

A `ChangeRecord` is `{ op: ChangeOp, data: <row JSON> }`. The row JSON **includes the key columns**, so a delete
carries its own key. `ChangeOp` is `Insert | Update | Delete`. Changes flow through the engine (and mapping) like
`Rows`, preserving op.

## Sync modes

`SyncMode` — how a source reads:

- `FullRefresh` — re-read everything each run.
- `Incremental` — read only what's new, tracked by a cursor.
- `Cdc` — stream inserts/updates/deletes from the source's change log.

## Write modes

`WriteMode` — how a destination applies records:

- `Append` — insert every record.
- `Upsert { business_keys }` — insert-or-update keyed on `business_keys`.
- `Overwrite` — replace the destination contents.

## Field types

A stream's typed schema is a list of fields, each `{ name, type, nullable }`. `FieldType` is one of `str`,
`integer`, `float`, `boolean`, `timestamp`, `json`. weir captures a schema per stream (declared or inferred) and
enforces it on the record path; see [Typed schemas](connection-config.md#typed-schemas).

## Sandbox

A connector runs as a `wasm32-wasip2` component: no filesystem, network egress only where the host grants a
capability (`http` or `tcp`), and **secrets never enter the guest** — the host injects credentials into outbound
requests. This is the security spine of the connector model; the *why* is in Explanation.
