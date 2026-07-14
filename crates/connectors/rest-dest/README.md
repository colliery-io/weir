# weir `rest-dest` — shared declarative destination runtime

The reverse-ETL analogue of [`crates/connectors/rest`](../rest): one WASM `wasi:http` guest
that interprets a **destination manifest** as config and upserts records into a SaaS REST API
([[WEIR-A-0034]]). No per-connector codegen — HubSpot, Salesforce, and the SaaS long tail are
manifests on this runtime.

Per record it: shapes the record into the object body (a `field_map` record-field → SaaS
property, plus an optional `body_wrap` like HubSpot's `properties`), templates the endpoint
(`{{ record.<field> }}` for upsert-by-key URLs), and issues the configured method (`POST` /
`PATCH`) over `wasi:http`. Auth is injected **host-side** ([[WEIR-A-0033]]); transient
failures (429 / 5xx) retry with backoff ([[WEIR-T-0069]]); a per-record 4xx dead-letters that
record instead of failing the sync.

Config keys: `base_url`, `path`, `method`, `field_map`, `body_wrap`, `max_retries`.
Capability: `http`. Built for `wasm32-wasip2`.
