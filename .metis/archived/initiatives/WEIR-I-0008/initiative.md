---
id: deeper-airbyte-declarative-parity
level: initiative
title: "Deeper Airbyte declarative parity (coverage + whole-catalog fidelity harness)"
short_code: "WEIR-I-0008"
created_at: 2026-06-22T01:35:25.645491+00:00
updated_at: 2026-07-04T03:13:38.222891+00:00
parent: WEIR-V-0001
blocked_by: []
archived: true

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: XL
initiative_id: deeper-airbyte-declarative-parity
---

# Deeper Airbyte declarative parity (coverage + whole-catalog fidelity harness) Initiative

*First initiative of the **parity arc** (strategy [[WEIR-A-0003]] migration-not-runtime; fidelity [[WEIR-A-0020]] tiered). The Python-CDK codemod/adapter is the sequenced follow-on (a later initiative). This is the ecosystem cold-start lever — "inherit the long tail by translation" ([[WEIR-V-0001]]) — and the "credible Airbyte replacement" success criterion.*

> **Sequencing — UNBLOCKED 2026-06-24:** the prerequisites are **done + archived** — [[WEIR-I-0011]]
> (native→wasm migration; wasm-only codegen) and [[WEIR-I-0010]] (the connector catalog + ingress = the
> emit→register→run target, incl. the `cargo build → stage → snapshot → register` pipeline a translated
> connector rides). [[WEIR-I-0009]] ✅ also landed. So **all of S1–S7 are now unblocked.** Every
> "dylib/wasm codegen" reference below means **wasm** ([[WEIR-A-0030]]); a translated connector is emitted
> as a `weir-connector-<name>` guest crate that the I-0010 ingress compiles + registers.
>
> **First pass (user direction, 2026-06-24): foundation only — S1 + S2.** Decompose + build S1 (in-flight
> mapping, independent; also unblocks [[WEIR-I-0007]]) and S2 (fidelity harness). S3–S7 stay planned here,
> decomposed when reached. **S2 corpus starts as a small *curated* MIT set** (hand-picked known-permissive
> declarative connectors) to build the harness against; the automated whole-catalog license filter is a
> follow-on refinement once the harness exists + corpus fetch is proven in this environment.

> **Progress 2026-06-25 — shared-runtime auth (S3 v1) + keyed harness (S2) landed, A-0032 model.**
> Under the interpreter model ([[WEIR-A-0032]]: vendored manifests run on the shared `rest` runtime, no
> per-connector codegen), auth landed **in the runtime**, not codegen: Bearer + header ApiKey are applied
> by `rest` (`Authorization: Bearer <key>` / `<header>: <key>`); the manifest declares the *scheme* and
> `manifest_stream_to_config` maps it (`auth_scheme`/`auth_name`), while the secret (`api_key`) is supplied
> per-connection and never baked into the manifest base. Proven over the real wasi:http wire
> (`wasm_http_source_sends_bearer_auth_header`) + a mapping unit test; the preview/coverage report no longer
> flags bearer/api-key as "not applied" (config requirement, not unsupported construct). Survey of the 34
> vendored manifests: 8 no-auth, 26 authed (30 bearer + 19 api-key usages, 3 query-param) — so this unblocks
> ~23 of 26. **Remaining gaps:** query-param api-keys (`?api_key=`, 3 manifests; needs `inject_into` on the
> importer + an `Auth` query variant), templated `url_base` (tenant subdomain / account id), offset
> pagination. **Keyed live harness** (`keyed_manifests_run_live`) added per Airbyte's `secrets/config.json`:
> a gitignored `secrets/<slug>.json` config overlay runs that authed connector E2E on the shared runtime
> (rows>0), skipped when absent so the harness grows as keys land; tracked `secrets/*.example.json` templates
> + README. Also renamed the base connector `rest-ref → rest` (REST). Commits 13d34b3, 7d59fbb, 7abe71a.

> **Progress 2026-06-25 (cont.) — three runtime gaps closed (commit ed8d3db).** Following the auth v1
> increment, the named gaps are closed so authed / multi-page / multi-tenant connectors function on the
> shared `rest` runtime: **(1) query-param api-keys** — new `Auth::ApiKeyQuery`; importer parses Airbyte's
> structured `inject_into` (`{field_name, inject_into: header|request_parameter}`), routing
> `request_parameter` → query else header; `manifest_stream_to_config` → `auth_scheme=query`; runtime
> appends `?<name>=<key>`. nasa (`?api_key=`) + ticketmaster (`?apikey=`) updated from the header-mode
> placeholder their own authoring comments flagged as incorrect, to real `inject_into`. **(2) Templated
> `url_base`** — the runtime renders `{{ config['KEY'] }}` in base_url/path against the per-connection
> config (tenant subdomain / account id / base url): gitlab, jira, mediawiki, zendesk. **(3) Offset
> pagination** — the runtime walks `?offset=N` advancing by page_size and terminating on the short page,
> instead of degrading to page-increment (~16 manifests); `Pagination::Offset` maps to
> `offset_param`+`page_size_param`. Coverage report no longer flags offset / api-key. Tests (green): wire-level
> query-param-key / templated-url_base / offset-pagination over real wasi:http + a query mapping unit test;
> full workspace lib suite green. **Remaining declarative gaps:** cursor (token) pagination + single-object
> (non-array) responses (e.g. xkcd), plus the S3+ deeper slices (OAuth/session, substream routers,
> transformations, error handlers) still planned below.

> **Progress 2026-06-26 — cursor/token pagination + single-object responses closed (commit 06e48a1).**
> The last two declarative pagination/shape gaps are closed on the shared `rest` runtime: **(1) opaque
> cursor pagination** — new `Pagination::Cursor`; importer `CursorPagination` strategy with `response_path()`
> extracting the dot-path from `{{ response['a']['b'] }}`; `manifest_stream_to_config` →
> `page_cursor_path`/`page_cursor_param`; the runtime reads the next-page token from the response body and
> sends `?<param>=<token>`, stopping when absent/empty. slack (`response_metadata.next_cursor`), square
> (top-level `cursor`), intercom (`pages.next.starting_after`) updated from the paginator-omitted
> placeholders their authoring comments prescribed. **(2) single-object responses** — a `record_path` that
> resolves to a JSON object (not array) on page 1 is emitted as one record (xkcd `/info.0.json`). Tests
> (green): wire-level cursor (2+2+1 across 3 token pages) + single-object over real wasi:http; cursor mapping
> unit test; xkcd promoted to the asserted live functional set. **Now-remaining declarative gaps:** POST-with-
> body requests (Notion body cursor), `Link`-header pagination (GitHub), and the S3+ slices (OAuth/session,
> substream routers, transformations, error handlers). The shared-runtime low-code surface now covers:
> bearer/header/query auth, page/offset/cursor pagination, templated url_base, datetime incremental,
> single-object + array record selection.

## Context **[REQUIRED]**

`weir-importer` today translates a thin slice of Airbyte's declarative (low-code) spec — `Bearer`/`ApiKey` auth, basic page/offset pagination, `DatetimeBasedCursor`, a record selector ([[WEIR-T-0012]]/[[WEIR-T-0013]]). Real connectors need far more: OAuth/session auth, cursor pagination, substream partition routers, transformations, error handlers, request bodies. This initiative deepens importer + manifest + codegen across the declarative surface, **measured against the whole Airbyte low-code catalog** via a fidelity harness.

The contract already supports the targets: partitioning (`PartitionScheme`, [[WEIR-A-0012]]), retry/dead-letter ([[WEIR-T-0008]]), and the in-flight mapping spec ([[WEIR-A-0026]]) that Airbyte transformations map onto. Parity proves these against real connectors.

## Goals & Non-Goals **[REQUIRED]**

**Goals:**
- A **whole-catalog fidelity harness** ([[WEIR-A-0020]]): vendor Airbyte's declarative connector manifests as a test corpus, run the importer over **all** of them, and report **tier + confidence + pass-rate** (a coverage dashboard) — never silently emit a broken connector.
- Deepen declarative coverage so a meaningful fraction of the catalog imports **and runs**: auth (OAuth/session), pagination + incremental, partition routers, transformations + request options, error handling.
- Land the shared **in-flight mapping stage** ([[WEIR-A-0026]] v0) — Airbyte transformations need it (and reverse-ETL [[WEIR-I-0007]] reuses it).

**Non-Goals:**
- **Python CDK** connectors — the sequenced follow-on initiative (codemod + adapter via fidius's Python backend).
- Java/Kotlin DB/warehouse connectors — out of scope for migration ([[WEIR-A-0020]]); built first-party.
- 100% automated coverage — tiered by design; some connectors flag for manual porting with confidence reported.
- Connector-builder UI ([[WEIR-A-0017]]/[[WEIR-A-0024]]).

## Detailed Design **[REQUIRED]**

### Fidelity-corpus / IP gate (HARD prerequisite of S2)
**MIT-only, programmatically enforced — not a "check later".** Airbyte relicensed in a connector-vs-platform split; the connector tree is *mostly* permissive but **not uniformly**, so we never bulk-vendor. The corpus is built by an **automated license filter** that, per source connector, reads its `metadata.yaml` `license` field (and any `LICENSE` file) and **includes only category-A permissive licenses (MIT, Apache-2.0, BSD)** — every other connector (ELv2, unknown, missing) is **excluded and logged**, never vendored. Only the `manifest.yaml` (declarative spec) of included connectors is taken — **not** the ELv2 platform, not Python/Java connector code.

Vendored **test-only** (a `corpus/` dir excluded from the shipped crates), each entry carrying its **upstream license + commit SHA provenance**, with aggregate **NOTICE attribution** ([[WEIR-V-0001]]'s IP-clean hard constraint). The filter's exclusion report (counts by license) is a build artifact. This gate runs **first in S2**; no manifest enters the repo without passing it.

### Slices
**S1 — In-flight mapping stage ([[WEIR-A-0026]] v0).** Engine-owned mapping between read and write: extend `MappingOp` with `Cast`/`Filter`/`Compute` + the bounded expression AST + evaluator; apply `MappingSpec` in `weir-engine`; row-JSON evaluator + Arrow projection/filter; dbt-boundary guard. *(Shared with [[WEIR-I-0007]].)*

**S2 — Whole-catalog fidelity harness ([[WEIR-A-0020]]).** Vendor the MIT declarative manifests (test-only + NOTICE); a harness that runs the importer over the corpus and emits per-connector **tier + confidence + pass/fail** + an aggregate coverage report; recorded-HTTP record-fidelity checks for a curated subset (assert weir emits the same records). The gate every coverage slice reports against.

**S3 — Auth coverage.** `OAuthAuthenticator` (refresh-token / client-credentials) + `SessionTokenAuthenticator` → weir `Auth` + a **host-side token provider** on the egress policy (the OAuth-refresh seam Salesforce reuses later). Keeps secrets host-side ([[WEIR-A-0013]]).

**S4 — Pagination + incremental coverage.** `CursorPagination` / page-token strategies; richer `DatetimeBasedCursor` (datetime formats, start/end datetime, step, lookback window). Map onto the manifest + dylib/wasm codegen pagination + `Incremental`.

**S5 — Partition routers.** `SubstreamPartitionRouter` (parent-stream → child slices) → `PartitionScheme::ByParent`; `ListPartitionRouter` → shards. Wires Airbyte parent-stream slicing onto the engine's partition planning ([[WEIR-A-0012]] / [[WEIR-T-0027]]).

**S6 — Transformations + request options.** `AddFields`/`RemoveFields`/key transforms → `MappingSpec` (uses S1); `request_parameters` / `request_body_json|data` / `request_headers`; `record_filter`.

**S7 — Error handling.** `DefaultErrorHandler` + backoff strategies (constant/exponential, response filters) → weir retry/backoff + dead-letter ([[WEIR-T-0008]]).

### Feed-forward
S1 + S2 are the foundation (mapping + the measurement gate); S3–S7 are coverage expansions, each landing with its corpus pass-rate delta. The harness's coverage report becomes the live "parity %" metric.

## Alternatives Considered **[REQUIRED]**

- **Point at the Airbyte repo at test time** instead of vendoring. Rejected — network-dependent, version-drifty, weaker IP provenance than a vendored, attributed, pinned corpus.
- **Synthetic feature manifests** only. Rejected as the primary corpus — doesn't prove real-connector fidelity (the credibility claim); used as supplementary targeted tests.
- **Python CDK first.** Deferred — declarative is bounded + high-leverage; CDK is the larger follow-on.
- **Thin coverage (auth+pagination only).** Rejected per direction — full S1–S7 declarative coverage before moving to CDK.

## Implementation Plan **[REQUIRED]**

S1 → S2 (foundation) → S3, S4, S5, S6, S7 (coverage, each gated by + reported through S2). Decompose into tasks after review. Each slice ends green (workspace + conformance); coverage slices report a corpus pass-rate delta.

## Exit Criteria

- [ ] In-flight mapping stage live ([[WEIR-A-0026]] v0): select/drop/rename/cast/filter/compute on row-JSON + Arrow; dbt boundary enforced.
- [ ] Whole-catalog harness vendors the MIT declarative corpus (test-only + NOTICE) + reports tier/confidence/pass-rate.
- [ ] Auth (OAuth/session), pagination/incremental, partition routers, transformations/request-options, error handling translated + codegen'd; each with corpus pass-rate gains.
- [ ] A curated subset runs E2E (recorded-HTTP fidelity) producing correct records.
- [ ] Workspace + integration suites green; clippy clean. Python-CDK initiative queued as the follow-on.
