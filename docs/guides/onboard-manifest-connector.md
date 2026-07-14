# Onboard a declarative connector

weir ships a corpus of **declarative connectors** — small YAML *manifests* that describe a REST source and run
on a shared WASM runtime, no compilation needed. Before a connection can use one, you **onboard** it: weir bakes
the manifest onto the runtime and registers it in the catalog. This is done over the HTTP API.

**Goal:** onboard the `exchangerate` manifest and pull from it.

## 1. Have the API running with a key

```bash
weir --db weir.db init                 # prints an admin key — copy it
export WEIR_MANIFESTS_DIR="$PWD/manifests"
weir --db weir.db api --port 8080 &
KEY=weirk_…                            # the key from init
```

All calls carry the key as a bearer token.

## 2. Import the manifest

```bash
curl -s -X POST http://localhost:8080/catalog/import \
  -H "authorization: Bearer $KEY" -H "content-type: application/json" \
  -d '{"manifest_name": "exchangerate"}'
```

The connector now appears in the catalog (`GET /catalog`). You can preview what a manifest exposes before
importing with `POST /catalog/preview`, and browse the vendored corpus in `manifests/` (`exchangerate.yaml`,
`coingecko.yaml`, …). Full-code (crate) connectors are imported the same way, by `{"package": "weir-<name>-pkg"}`.

## 3. Create a connection off it

```bash
curl -s -X POST http://localhost:8080/connections \
  -H "authorization: Bearer $KEY" -H "content-type: application/json" \
  -d '{"name":"rates","source":"exchangerate","dest":"ArrowSink","stream":"latest","config":{}}'
```

## 4. Run it

```bash
curl -s -X POST http://localhost:8080/connections/rates/run -H "authorization: Bearer $KEY"
```

**Done** when `GET /connections/rates/runs` shows a run in state `done` with `rows_written > 0`.

## Notes

- Declarative connectors are for REST sources; a destination manifest (reverse-ETL) onboards the same way from
  `WEIR_DEST_MANIFESTS_DIR` and is used as a connection's `dest`.
- Authenticated sources need a secret — weir consumes rotated secrets, it does not manage them. See
  [Secure the control plane](secure-control-plane.md) for how keys and auth are handled.
- To author your own full-code connector, see [Author a connector](connector-authoring.md).
