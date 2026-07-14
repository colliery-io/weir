# Authoring a connector

A weir connector is a **WASM component** (`wasm32-wasip2`) that implements one trait. It runs sandboxed:
its only reach to the outside world is **network egress** (`http` or `tcp`) that the host authorizes — there is
**no filesystem access**, and **secrets never enter the guest** (the host signs/injects them on the way out,
[[WEIR-A-0033]]). This guide walks the **`s3` object-store source** (`crates/connectors/s3`) as the worked
example. Use `angreal connectors new <name>` to stamp a fresh crate from the template, then fill in the four
methods below.

## Anatomy

```
crates/connectors/<name>/
  Cargo.toml        # a standalone [workspace]; crate-type = ["cdylib"]; declares capabilities
  build.rs          # fidius_build::emit_wit()  (generated — leave it)
  wit/connector.wit # the shared connector contract (identical across connectors)
  src/lib.rs        # the shared WIT-type block  +  YOUR impl
  README.md
```

`Cargo.toml` declares what egress the host must grant:

```toml
[package.metadata.weir]
capabilities = ["http"]   # or ["tcp"], or [] for none
```

`src/lib.rs` is two parts. The **top** (through the `Connector` trait) is boilerplate shared verbatim by every
connector — the WIT type definitions and the `#[plugin_interface]` trait. Don't edit it. The **bottom** is
yours: a struct plus `#[plugin_impl] impl Connector for <T>`.

## The four methods

```rust
fn spec(&self) -> ConnectorSpec;                        // name, roles, supported sync modes, config JSON-schema
fn check(&self, config: Config) -> CheckResult;         // cheap validity probe
fn discover(&self, config: Config) -> DiscoverOutcome;  // the streams this connector exposes
fn read(&self, ctx: ReadContext) -> Stream<ReadMessage>;// a Source produces records + a checkpoint
```

(A destination implements `write` instead of `read`.)

- **`spec`** — `roles: vec![ConnectorRole::Source]`, `supported_sync_modes`, and a JSON-schema for the config
  the operator fills in. The s3 source needs `endpoint`, `bucket`, `prefix`.
- **`discover`** — advertise the stream(s). s3 exposes one `objects` stream with a source-defined cursor (the
  object key).
- **`read`** — the heart. Return `ReadMessage::Records(RecordBatch::Rows(..))` (each row a JSON string) followed
  by `ReadMessage::Checkpoint(StreamState { cursor, .. })`. Honour `ctx.stream.sync_mode` and resume from
  `ctx.state.cursor`.

## Egress, and never handling secrets

The guest builds **plain, unsigned** requests:

```rust
let resp = fidius_guest::http::send(fidius_guest::http::Request::get(url))?;
let body = resp.text();
```

The **host** attaches credentials in the egress policy — a static header, an OAuth/session token, or a full
**AWS SigV4 signature** — based on the connection's `auth_scheme`. For s3 that's `auth_scheme: "aws_sigv4"`; the
host's `Credential::AwsSigV4` signs each outbound request and the access/secret keys are stripped from the
config before it ever reaches the guest. **Your connector code never sees a key** — design for that.

## Incremental & cursors

Return a `cursor` in the checkpoint; you get it back on the next run as `ctx.state.cursor`. s3 uses the greatest
object key read (with S3 `start-after`), so append-only key schemes (date-partitioned, ULID) resume cleanly. A
REST source uses a datetime high-watermark; Postgres uses a column value or a logical-replication slot.

## Build & test

```sh
# the wasm builds on wasm32-wasip2 (opt-level "s", lto, strip):
cargo build --release --target wasm32-wasip2      # inside crates/connectors/<name>

# an engine-level test stages the wasm + drives it through the real Engine → a sink.
# See crates/weir-engine/tests/wasm_s3_engine.rs — it wires HostAllowList::with_credential(...)
# so the host signs the guest's requests, then asserts rows land at an ArrowSink.
cargo test -p weir-engine --test wasm_s3_engine -- --ignored --test-threads=1
```

The `#[ignore]` tests that need live infra (MinIO, Postgres) run in the integration lane
(`angreal integration up` then the test). Unit-testable logic (parsing, cursoring) should be plain `#[test]`s.

## Checklist

- [ ] `capabilities` declared in `Cargo.toml` match what you actually call (`http`/`tcp`/none).
- [ ] `spec` config-schema lists every operator-facing field; **no** secret/auth fields (host-side).
- [ ] `read` honours `sync_mode` + resumes from `ctx.state.cursor`, and emits a `Checkpoint`.
- [ ] An engine test proves discover→read→rows; live-infra ones are `#[ignore]`.
- [ ] A `README.md` states roles, sync modes, config, and how auth is resolved host-side.
