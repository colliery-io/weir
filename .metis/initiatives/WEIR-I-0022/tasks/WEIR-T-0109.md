---
id: connector-authoring-hardening-file
level: task
title: "Connector-authoring hardening + file/object-store source"
short_code: "WEIR-T-0109"
created_at: 2026-07-06T11:33:29.329135+00:00
updated_at: 2026-07-07T00:21:14.457873+00:00
parent: WEIR-I-0022
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0022
---

# Connector-authoring hardening + file/object-store source

## Parent Initiative

[[WEIR-I-0022]] — closes the **connector breadth** partial (and chips the file/object-store **gap**).

## Objective

Prove the connector-authoring path is **production-ready** for a category beyond REST/DB: stand up a
**file/object-store source** end-to-end, and harden the authoring experience (scaffold + docs) so the next
connector is cheap.

## Reference

- `crates/connectors/{postgres,rest,rest-dest}` — the existing connector crates; the WIT + `SyncMode`/partition
  shape to mirror.
- `crates/connectors/postgres/README.md` — the per-connector README pattern.
- `angreal test connectors` — the wasm build path; `manifests/` — the onboarding corpus + validation.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [x] **Object-store source** `crates/connectors/s3` (guests can't touch the FS — sandbox; so S3-over-HTTP,
  not local-dir): ListObjectsV2 + GetObject, **NDJSON**, `FullRefresh` + `Incremental` by object-key cursor.
- [x] **Runs end-to-end** — `wasm_s3_engine.rs` (2 tests) reads NDJSON from **live MinIO**, host-signed with
  SigV4: full-refresh → 5 records; incremental-by-key resumes (5 then 0).
- [x] **Authoring hardening** — `angreal connectors new <name>` stamps a crate that **compiles to
  wasm32-wasip2** (verified); `docs/guides/connector-authoring.md` walks the s3 source as the worked example.
- [x] Wired into the build via on-demand `stage()` in its engine test (same precedent as `postgres`) +
  MinIO/seed in `compose.yml`; the e2e runs in the integration lane.
- [x] Workspace + clippy clean.

## Status Updates

### 2026-07-06 — key finding + stage 1 done (`6673dbf`)

**Architectural finding**: guests get only network egress (`authorize_tcp`/http) — **no filesystem capability**
([[WEIR-A-0030]] sandbox). So a *local-directory* source is impossible in the guest; the connector must be
**object-store over HTTP** (S3 API). User chose **full authenticated S3 with SigV4** (2026-07-06).

**Signing must be host-side** — SigV4 signs the whole request with the secret, but secrets never enter the guest
([[WEIR-A-0033]]). So the signer lives in the host egress path, not the connector.

**Stage 1 — host-side SigV4 credential: DONE + verified (`6673dbf`).**
- `weir-runtime/sigv4.rs`: pure signer, **verified against AWS's published test vector** (AKIDEXAMPLE IAM
  ListUsers → known signature) + empty-payload-hash + epoch→amz-timestamp.
- `Credential::AwsSigV4 { access_key, secret_key, region, service }` + `apply` (canonical URI/query encoders,
  injects x-amz-date/x-amz-content-sha256/Authorization). `from_auth_config` parses `auth_scheme=aws_sigv4` and
  **strips the keys** from the guest-facing config. Tests green; clippy clean.

**Remaining (stages 2–3), for resume:**
1. **Guest `crates/connectors/s3`** (mirror `rest`): unsigned GET `?list-type=2` → parse the ListObjectsV2 XML
   (Key + LastModified) → GET each object → NDJSON records; `FullRefresh` + `Incremental` (cursor by key/mtime).
   Host signs via the AwsSigV4 credential.
2. **MinIO** in `compose.yml` (integration) + a bucket of NDJSON fixtures; an engine/integration e2e proving
   discover→run→rows with host-side signing.
3. **Authoring hardening**: connector scaffold (angreal task/template) + `docs/guides/connector-authoring.md`
   using the S3 source as the worked example; wire into the wasm build.

Also: fix the pre-existing `wasm-fixtures/rest` gap (the `angreal test connectors` staging lists a `rest`
fixture dir that isn't present) if it blocks the connector build lane. — Not needed; the s3 connector stages
on-demand from `crates/connectors/s3` (like postgres), so it never touched that lane.

### 2026-07-06 — stages 2–3 done, T-0109 COMPLETE (`ec43dfe`, `55fecf1`)

**Stage 2 — guest s3 source + live MinIO e2e (`ec43dfe`).** `crates/connectors/s3` (wasm): ListObjectsV2 XML →
GetObject → NDJSON; FullRefresh + incremental by object-key (`start-after`). Fixed a SigV4 subtlety: sign the
**authority** (`host:port`) so MinIO on `:9000` validates. MinIO + a seed step in `compose.yml`. **Two e2e tests
pass live against MinIO** — full-refresh (5 records) + incremental-by-key resume (5 then 0). SigV4 now proven
against **both** AWS's vector and a real S3 endpoint.

**Stage 3 — authoring hardening (`55fecf1`).** `angreal connectors new <name>` stamps a connector crate from the
canonical template — **verified to compile to wasm32-wasip2**. `docs/guides/connector-authoring.md` (worked s3
example: the sandbox, the four methods, host-side auth, cursors, build+test, checklist) + nav.

**Complete — closes the connector-breadth partial + the file/object-store gap.**
