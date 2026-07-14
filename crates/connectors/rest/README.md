# weir-rest-wasm

A **REST/HTTP source connector** for [weir](https://github.com/colliery-io/weir), shipped as a
sandboxed **WASM component** (`wasm32-wasip2`). Config-driven and declarative — it maps directly onto
the Airbyte-style low-code shape (base URL + path + record selector + pagination + datetime cursor), so
one connector pulls from a wide range of JSON HTTP APIs without bespoke code.

> **Status: alpha (`0.0.1`).** Interfaces may change before `0.1`.

## Configuration

| Field | Required | Meaning |
|---|---|---|
| `base_url` | yes | Scheme + host (+ stable prefix), e.g. `https://api.coinpaprika.com/v1` |
| `path` | yes | Path for this stream, e.g. `/coins` |
| `record_path` | no | Dot-path to the records array (`data.items`); blank = top-level array |
| `page_param` | no | Query param for the page number; omit for a single, unpaginated request |
| `page_size_param`, `page_size` | no | Query param + value for page size |
| `cursor_field` | no | Record field whose max advances the incremental cursor (blank = full refresh) |
| `cursor_param` | no | Query param used to filter by the cursor |

### Example

```json
{
  "base_url": "https://api.coinpaprika.com/v1",
  "path": "/coins",
  "record_path": ""
}
```

Pulls the top-level JSON array from `GET /coins` and emits each record whole; weir's in-flight mapping
stage then shapes them (select / rename / cast / filter / compute) before the destination.

## Egress

The connector's `wasi:http` is governed by a **host egress policy** — the guest never sees credentials
or the allow-list; the host authorizes each request and can inject auth headers.

Licensed under Apache-2.0.
