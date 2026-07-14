---
id: declarative-parity-ledger-airbyte
level: specification
title: "Declarative parity ledger (Airbyte low-code construct coverage)"
short_code: "WEIR-S-0016"
created_at: 2026-06-28T01:46:30.763786+00:00
updated_at: 2026-06-28T01:46:30.763786+00:00
parent: WEIR-I-0008
blocked_by: []
archived: false

tags:
  - "#specification"
  - "#phase/discovery"


exit_criteria_met: false
initiative_id: NULL
---

# Declarative parity ledger (Airbyte low-code construct coverage)

*The target/current map for [[WEIR-I-0008]] (declarative parity). One rung below [[WEIR-S-0001]]
(product capabilities, areas A–M): this enumerates the **Airbyte low-code CDK construct surface** and
weir's support state for each, on the shared `rest` declarative runtime + `weir-importer`. It is the
finite gap list the parity arc burns down, and the target a connector author checks before authoring
against a construct.*

## Overview **[REQUIRED]**

weir's strategy for the Airbyte long tail is **translate-not-runtime** ([[WEIR-A-0003]]): vendored
`manifest.yaml`s run on one shared declarative runtime (`rest`), with `weir-importer` mapping the
Airbyte low-code spec onto weir's connector config ([[WEIR-A-0032]]). "Parity" is therefore not a vibe —
it is **the fraction of the bounded Airbyte low-code construct set that the runtime + importer cover**,
construct by construct.

This ledger is that count, made explicit. Each **row is one Airbyte low-code construct** (an
authenticator, a paginator, a partition router, a record-selection/transform/error construct, a request
option), with its **current support state**, the **slice/task** that delivers it, and the **vendored
connectors that exercise it**. The `current` column is the live coverage number; the ❌ rows are the
work queue.

**This is a planning/reference spec, not a feature.** Fidelity (does a *specific* connector emit the
*right records*) is the long-tail outcome chased per-connector later, gated by the future fidelity
harness ([[WEIR-T-0053]]); this ledger is the construct-level *target* that harness will eventually
verify the *current* column against automatically. Capabilities first (close construct gaps), author
connectors against them, fidelity converges toward this target.

## State legend

- **✅ supported** — runtime + importer handle it; proven by a wire-level test over real `wasi:http`.
- **⚠️ partial** — basic case handled; named sub-cases missing (noted in the row).
- **❌ not yet** — importer reports it as unsupported (tier/confidence), never silently drops it.

## Coverage ledger **[REQUIRED]**

State as of 2026-06-28 (commit `06e48a1` + the demo arc). `Slice` references the [[WEIR-I-0008]]
slice; task short codes are filled as slices are decomposed.

### Auth (authenticators)

| Construct (Airbyte) | weir | Current | Slice / task | Exercised by |
|---|---|---|---|---|
All auth is now injected **host-side** ([[WEIR-A-0033]]); the secret never enters the guest.

| Construct (Airbyte) | weir | Current | Slice / task | Exercised by |
|---|---|---|---|---|
| `NoAuth` | — | ✅ | done | frankfurter, jsonplaceholder, rickandmorty, xkcd, coinpaprika |
| `ApiKeyAuthenticator` (header) | `Credential::Header` (host) | ✅ | [[WEIR-T-0063]] | ~19 manifests |
| `ApiKeyAuthenticator` (query, `inject_into=request_parameter`) | `Credential::Query` (host uri-rewrite) | ✅ | [[WEIR-T-0063]] | nasa, ticketmaster, 1 other |
| `BearerAuthenticator` | `Credential::Header` (host) | ✅ | [[WEIR-T-0063]] | ~30 manifests |
| `BasicHttpAuthenticator` | `Credential::Header` (host, `Basic base64(user:pass)`) | ✅ | [[WEIR-T-0070]] | (catalog: several) |
| `OAuthAuthenticator` (refresh-token / client-credentials) | `Credential::OAuth2` (host grant + expiry refresh) | ✅ | [[WEIR-T-0063]] | (catalog: large authed set) |
| `SessionTokenAuthenticator` | `Credential::Session` (host login) | ✅ | [[WEIR-T-0063]] | (catalog: session-token APIs) |
| `SelectiveAuthenticator` / custom | — | ❌ | later | — |

*Refresh is expiry-based (re-mint within a 60s margin of `expires_in`); 401-driven refresh is a noted follow-up ([[WEIR-A-0033]]). Session-token v1 performs a plain login `POST`; credentialed login bodies are a follow-up.*

### Pagination (paginators)

| Construct | weir | Current | Slice / task | Exercised by |
|---|---|---|---|---|
| No pagination (single page) | — | ✅ | done | xkcd, frankfurter |
| `PageIncrement` | `Pagination::Page` | ✅ | done | rickandmorty (~7 pages) |
| `OffsetIncrement` | `Pagination::Offset` | ✅ | done | ~16 manifests |
| `CursorPagination` (response-body token) | `Pagination::Cursor` | ✅ | done | slack, square, intercom |
| `CursorPagination` via `Link` header | `Pagination::LinkHeader` (rest follows `rel="next"`) | ✅ | [[WEIR-T-0070]] | github |
| `DefaultPaginator` page-size param | (param mapping) | ✅ | done | — |

### Incremental (cursors)

| Construct | weir | Current | Slice / task | Exercised by |
|---|---|---|---|---|
| `DatetimeBasedCursor` (basic) | `Incremental` + cursor_field | ✅ | done | — |
| `DatetimeBasedCursor` richer (start/end datetime) | rest: `cursor_start` lower bound + `cursor_end` upper bound | ✅ | [[WEIR-T-0070]] | — |
| `DatetimeBasedCursor` step-windowing / lookback / custom formats | — | ❌ | later (reported) | — |
| Custom / `CustomIncrementalSync` | — | ❌ | later | — |

### Partition routers

| Construct | weir | Current | Slice / task | Exercised by |
|---|---|---|---|---|
| Unpartitioned (single slice) | `PartitionScheme::Unpartitioned` | ✅ | done | most |
| `ListPartitionRouter` | `rest` runtime: one request per static value | ✅ | [[WEIR-T-0064]] | (catalog: list-sliced APIs) |
| `SubstreamPartitionRouter` (parent→child slices) | `rest` runtime: read parent → one request per parent key | ✅ | [[WEIR-T-0064]] | (catalog: substream APIs) |
| `CustomPartitionRouter` / templated values / `$ref` parent | — | ❌ | later (reported, not silently dropped) | — |

*Routing is connector-internal in the shared `rest` runtime (not the engine's parallel `PartitionScheme`): the slice value is templated into the request as `{{ stream_partition.* }}` and records are concatenated. Parent keys are materialized (fine for phase-1 declarative scope).*

### Record selection & transforms

| Construct | weir | Current | Slice / task | Exercised by |
|---|---|---|---|---|
| `RecordSelector` `field_path` (array response) | record_path | ✅ | done | most |
| Single-object (non-array) response | record_path → object | ✅ | done | xkcd |
| `record_filter` (simple `record['f'] OP v`) | `MappingOp::Filter` | ✅ | [[WEIR-T-0071]] | — |
| `AddFields` (literal / `{{ record['f'] }}`) / `RemoveFields` | `MappingOp::{Compute,Drop}` (reuses [[WEIR-T-0052]]) | ✅ | [[WEIR-T-0071]] | — |
| Complex transform grammar (jinja concat/arithmetic/filters, compound conditions) | — | ❌ | later (reported) | — |

### Request options & shapes

| Construct | weir | Current | Slice / task | Exercised by |
|---|---|---|---|---|
| `request_parameters` (query) | query mapping | ✅ | done | — |
| `request_headers` (static) | rest: static headers on the request | ✅ | [[WEIR-T-0068]] | notion (`Notion-Version`) |
| `request_body_json` / `request_body_data` (POST body) | rest: POST body (config-templated) | ✅ | [[WEIR-T-0068]] | notion |
| HTTP method `POST` | rest: `http_method` | ✅ | [[WEIR-T-0068]] | notion |
| Body-cursor pagination (cursor in the POST body — full Notion) | — | ❌ | later (reported) | notion |

### Error handling

| Construct | weir | Current | Slice / task | Exercised by |
|---|---|---|---|---|
| Transient-error retry (429 / 5xx / transport) | rest: default retry (exp backoff + `Retry-After`), always on | ✅ | [[WEIR-T-0069]] | universal |
| Backoff (exponential + `Retry-After`) | rest: `backoff_ms` + `retry_after_ms`, `max_retries` (default 4) | ✅ | [[WEIR-T-0069]] | — |
| `DefaultErrorHandler` custom response-filter rules (per-status action, error substrings) | — | ❌ | later (default retry covers 429/5xx) | — |
| `CompositeErrorHandler` / constant / wait-time-from-body | — | ❌ | later | — |

## Requirements **[REQUIRED]**

### Functional Requirements

| ID | Requirement | Rationale |
|----|-------------|-----------|
| REQ-1 | Every Airbyte low-code construct weir intends to support appears as a row with an explicit current state (✅/⚠️/❌). | The ledger is only a true coverage number if the denominator (the construct set) is enumerated, not implied. |
| REQ-2 | A slice that adds construct coverage MUST flip the corresponding row(s) to ✅ as part of its definition-of-done, in the same change. | Keeps target and reality from drifting; the ledger stays trustworthy without a separate audit. |
| REQ-3 | Unsupported constructs are reported by `weir-importer` (tier/confidence), never silently dropped — the ❌ rows correspond to importer-detectable gaps. | [[WEIR-A-0020]]: never silently ship a broken connector. |
| REQ-4 | The `current` column is the source for the headline "parity %" (`✅ count / total constructs`). | One defensible number for the credible-replacement claim, derived not asserted. |

### Non-Functional Requirements

| ID | Requirement | Rationale |
|----|-------------|-----------|
| NFR-1 | The future fidelity harness ([[WEIR-T-0053]]) populates/verifies the `current` column by running the importer over the corpus; at that point this ledger becomes the hand-curated **target** and the harness owns **current**. | Closes the loop from hand-maintained to machine-verified without restructuring the doc. |
| NFR-2 | Construct names track the Airbyte low-code CDK component names so a porter can cross-reference upstream. | The ledger doubles as the migration cross-reference. |

## Constraints **[CONDITIONAL: Has Constraints]**

### Technical Constraints
- Scope is the **declarative / low-code** surface only. Python-CDK custom components and Java/Kotlin DB/warehouse connectors are out of scope here ([[WEIR-A-0020]] tiers; CDK is the sequenced follow-on initiative).
- Coverage is delivered on the **shared `rest` runtime + importer** ([[WEIR-A-0032]]) — no per-connector codegen. A row is ✅ only when the runtime executes it, not merely when the importer parses it.

## Decision Log **[CONDITIONAL: Has ADRs]**

| ADR | Title | Status | Summary |
|-----|-------|--------|---------|
| [[WEIR-A-0003]] | Airbyte compatibility strategy | decided | Translate-not-runtime: migrate manifests, don't embed Airbyte's runtime. |
| [[WEIR-A-0020]] | Migration translation fidelity | decided | Tiered fidelity; report tier/confidence, never silently emit broken. |
| [[WEIR-A-0032]] | Connector distribution (source-only) | decided | Low-code manifests run on a shared declarative runtime; this ledger measures that runtime's reach. |
| [[WEIR-A-0033]] | Host-side credential injection | decided | All auth injected host-side; secrets never enter the guest. Realized in [[WEIR-T-0063]]. |

## Changelog **[REQUIRED after publication]**

| Date | Change | Rationale |
|------|--------|-----------|
| 2026-06-28 | Initial ledger seeded from current state (commit `06e48a1` + demo arc). | Establish the target/current map driving [[WEIR-I-0008]]. |
| 2026-06-29 | OAuth2 + session-token rows ❌→✅; bearer/header/query rows moved host-side ([[WEIR-T-0063]] / [[WEIR-A-0033]]). | Auth coverage landed + migrated to host-side injection. |
| 2026-06-30 | List + substream partition-router rows ❌→✅ ([[WEIR-T-0064]]). | Partition routing landed (connector-internal in the `rest` runtime). |
