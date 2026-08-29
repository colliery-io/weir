# weir

**An open-source, no-code data ingestion & reverse-ETL platform.** weir runs continuous
pipelines — from APIs, databases (including CDC), and SaaS services into warehouses, and from
the warehouse back out to operational systems — through a web UI, with no code required.

- **Rust control plane**: scheduler (intervals + cron), durable work queue with leases,
  long-lived *resident* sources for continuous polling/event tailing, health dashboards.
- **Sandboxed connectors**: every connector is a WASM component behind a capability-gated
  egress policy; credentials are injected host-side and never enter the connector sandbox.
- **Two ways to build connectors**: compiled Rust guests (Postgres incl. CDC, MSSQL,
  Snowflake, S3) or declarative YAML manifests on a shared REST runtime (30+ vendored, plus
  an Airbyte low-code importer).
- **Reverse ETL first-class**: HubSpot and Salesforce destinations ship in the core.
- **Apache-2.0, all of it** — built in the open to be donated to the Apache Software
  Foundation. The open core is the whole product, not a teaser.

> **Status: alpha.** The core loop works and is well tested; the edges are still being
> finished. Expect breaking changes between releases (the API is v0/unstable).

## Quickstart (Docker)

```bash
git clone https://github.com/colliery-io/weir.git
cd weir
docker compose --profile demo up --build
```

Then open **http://localhost:8080**. On the first start against a fresh store, weir mints an
admin API key and prints it **once** in the `weir` service output — save it and use it to
sign in:

```
fresh store — admin API key minted; save this, it is not shown again:
  weirk_…
```

(Missed it? `docker compose --profile demo logs weir | grep -A1 "admin API key"`.)

## Quickstart (from source)

See [Installation](docs/reference/installation.md), then follow
[Your first sync](docs/tutorials/first-sync.md) — a complete ingest, end to end, in a few
minutes. The full documentation (tutorials, how-to guides, reference, architecture) lives in
[`docs/`](docs/).

## Repository layout

```
crates/
  weir-cli/            # the `weir` binary (serve, api, runner, autoscaler, auth, connections)
  weir-api/            # HTTP control plane (axum) + embedded web UI
  weir-app/            # application core: connections, tenants, auth, ingress, health
  weir-orchestrator/   # scheduler, durable work queue, leases, autoscaler
  weir-engine/         # sync engine: checkpointed reads, mapping, schema enforcement
  weir-runtime/        # WASM host: egress policy, host-side credential injection
  weir-connector*/     # connector contract + shared types
  weir-manifest/       # declarative connector manifests (model)
  weir-importer/       # Airbyte low-code → weir manifest importer
  connectors/          # the WASM connector guests (own workspace, wasm32-wasip2)
weir-ui/               # the Leptos web UI (embedded into the binary by trunk)
manifests/             # vendored declarative source connectors
dest-manifests/        # vendored declarative destinations (HubSpot, Salesforce)
docs/                  # Diátaxis documentation tree
charts/                # Helm charts (server + per-tenant runner)
```

## Development

This project uses [angreal](https://github.com/angreal/angreal) for task automation
(`pip install angreal`).

```bash
angreal check all          # fmt + clippy
angreal test unit          # unit tests
angreal test connectors    # build the WASM guests (compile-once cache for the suite)
angreal test functional    # engine-level functional tests
angreal integration up     # Postgres/MSSQL/MinIO/Dex for the #[ignore]d integration tests
angreal docs serve         # docs site with live reload
angreal tree               # everything else
```

Pre-commit hooks: `pip install pre-commit && pre-commit install`.

## License

[Apache-2.0](LICENSE). Contributions welcome — see [CONTRIBUTING.md](CONTRIBUTING.md).
weir is developed in the open with the intent of donation to the Apache Software Foundation;
everything in this repository is and will remain Apache-2.0.
