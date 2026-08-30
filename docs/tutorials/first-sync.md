# Your first sync

In this tutorial you'll run weir end to end: install it, define a **connection** from a source to a destination,
run it, and see your records land in a Postgres table you can query. It takes about ten minutes and leaves you
with a working mental model of how weir moves data.

You will:

1. install weir and stage its connectors,
2. start a Postgres to land data in,
3. define and run a connection,
4. look at the rows that arrived.

## Before you start

You need **Rust 1.93+** and **Docker** (for the Postgres destination). Install weir and stage the connectors as
described in [Installation](../reference/installation.md):

```bash
cargo install --path crates/weir-cli
bash scripts/stage-connectors.sh ./connectors
export WEIR_CONNECTORS_DIR="$PWD/connectors"
```

## 1. Start a Postgres

weir ships a compose stack for local Postgres. Bring it up (this maps it to `localhost:5433` so it won't clash
with any Postgres you already run):

```bash
WEIR_PG_HOST_PORT=5433 angreal integration up
```

## 2. Initialise weir

weir keeps its control-plane state in a local database. Create one, and mint an admin key while you're at it:

```bash
weir --db weir.db init
```

```
initialized store at weir.db

  admin API key — save this, it is not shown again:
    weirk_…
```

You won't need the key for this tutorial (the CLI talks to the store directly), but you would to call the
[HTTP API](../api/index.md).

## 3. Define a connection

A **connection** is a source, a destination, a stream name, and some config. Here the source is `slow` — a
built-in generator that emits a few `{ "n": … }` records, ideal for a first run — and the destination is
`postgres`:

```bash
weir --db weir.db connection add \
  --name first-sync \
  --source slow \
  --dest postgres \
  --stream demo_rows \
  --config '{"url":"postgres://weir:weir@localhost:5433/weir?sslmode=disable","rows":5,"batch":true}'
```

```
added connection `first-sync`
```

The `--config` is passed to the connectors: `rows`/`batch` tell `slow` how much to emit, and `url` tells the
`postgres` destination where to write. The `--stream` (`demo_rows`) becomes the destination table.
`sslmode=disable` is needed because the tutorial's Docker Postgres speaks plaintext — the connector
**defaults to `sslmode=require`** (TLS), which is what a managed database (RDS, Cloud SQL, Azure) wants;
`verify-full` adds certificate verification (optionally against an inline-PEM `sslrootcert`).

## 4. Run it

```bash
weir --db weir.db run --name first-sync
```

```
run #1870… -> done
```

`run` plans the work and drains it to completion in-process — the synchronous path. To run connections on a
schedule instead, give the connection an `--every` interval (seconds) and start the daemon with `weir serve`.

## 5. See your data

The five records are now rows in the `demo_rows` table:

```bash
PGPASSWORD=weir psql -h localhost -p 5433 -U weir -d weir -c "SELECT data FROM demo_rows;"
```

```
   data
----------
 {"n": 1}
 {"n": 2}
 {"n": 3}
 {"n": 4}
 {"n": 5}
(5 rows)
```

That's a complete ingest: a source produced records, weir moved them through its engine, and the destination
wrote them. `weir connection list` shows the connection you created.

## What you did

- A **connection** binds a source connector to a destination connector over a named stream.
- `weir run` executes it once; `weir serve` runs scheduled connections continuously.
- Connectors are sandboxed WASM components — `slow` and `postgres` here — that weir loaded from
  `WEIR_CONNECTORS_DIR`.

## Where to next

- **Real sources** — onboard a declarative REST connector and pull live data (How-to guides).
- **Shape records in flight** — [map fields](../guides/field-mapping.md) between source and destination.
- **Understand the machine** — how a sync flows through weir's engine and orchestrator (Explanation).

To tear down the Postgres when you're done: `WEIR_PG_HOST_PORT=5433 angreal integration down`.
