---
id: 001-streaming-configured-instance
level: adr
title: "Streaming + configured-instance connector contract (fidius 0.5)"
number: 1
short_code: "WEIR-A-0029"
created_at: 2026-06-21T00:39:05.975056+00:00
updated_at: 2026-06-21T00:42:55.030344+00:00
decision_date: 2026-06-20
decision_maker: Dylan Storey
parent:
archived: false

tags:
  - "#adr"
  - "#phase/decided"


exit_criteria_met: false
initiative_id: NULL
---

# ADR-0029: Streaming + configured-instance connector contract (fidius 0.5)

> **RATIFIED 2026-06-20 (Dylan Storey).** Amends [[WEIR-A-0014]] / [[WEIR-S-0006]]. Implementation tracked in [[WEIR-I-0006]], **gated** on fidius 0.5.0 → crates.io and the weir 0.4.2 → 0.5.0 bump ([[WEIR-I-0005]]); decompose/activate after gates clear.

## Context **[REQUIRED]**

[[WEIR-I-0004]] made the engine **honor** the contract dimensions the types already declared (write modes, incremental, partitioned reads, CDC) — verified live against Postgres, no contract change. But the **data path itself** is still the **chunked-pull** model:

```
read(ReadContext{ config, stream, partition, state }) -> ReadOutcome{ batch, next_state, has_more }
```

The engine loops while `has_more`, feeding `next_state` back in; `config` is re-marshaled into every call. [[WEIR-S-0006]] states this is the shape **"because fidius 0.3.0 is request/response, not streaming"** — an explicit workaround, not the intended design.

**fidius 0.5.0 removes that constraint.** It adds, all macro-declarable across cdylib/WASM/Python:
- **Streaming in three directions** — server (`-> fidius::Stream<T>`), client (`Stream<T>` arg), bidirectional — pull-based, backpressured, drop-to-cancel, with a lazy host producer (unbounded input, bounded memory).
- **Configured plugin instances** — bind config **once** at construction (`config = C` + `configure(cfg) -> Self`); host `load_wasm_configured::<C>` / `configure_in_process::<C>`. fidius's own motivating example is *"a REST connector configured with `{url, page_size, credentials}`"* whose *"`read()` returns a stream."*
- **`fidius_guest::http`** — brokered outbound HTTP from a macro connector (unblocks [[WEIR-T-0023]]; tracked under [[WEIR-I-0005]], not this ADR).

So the contract can now be what [[WEIR-A-0014]] wanted before the 0.3.0 workaround. This ADR decides whether — and how — to evolve it.

## Decision **[REQUIRED]**

Evolve the connector contract from chunked-pull/config-per-call (**v0**) to **streaming + configured instances (v1)**:

1. **Streaming `read`.** Replace the `has_more` loop with a server-streaming method:
   ```
   read(ReadContext) -> Stream<ReadMessage>
   ReadMessage = Records(RecordBatch) | Checkpoint(StreamState) | Log(LogEntry) | DeadLetter(record, error)
   ```
   The connector yields records and **inline `Checkpoint` messages** (it owns checkpoint granularity, à la Airbyte state messages — the typed `Message` enum [[WEIR-S-0006]] already resolved, now as a real stream). The engine maps ([[WEIR-A-0026]]) + writes each batch, and **commits on `Checkpoint`** transactionally ([[WEIR-A-0011]]). Backpressure = the engine pulls as it drains downstream; cancel/shutdown = drop the stream.

2. **Configured instances.** A connector binds typed config **once** at construction (`configure(Config) -> Self`, opening its client/connection once). `ReadContext`/`WriteContext` **drop the per-call `config`**. Lifecycle:
   - `spec()` — static, no instance.
   - `check(config)` / `discover(config)` — pre-construction validation/introspection (cheap, one-shot; a transient configured-or-unit instance).
   - `read` / `write` — run on a **configured instance** built with the validated config (`load_wasm_configured` / `configure_in_process`).

3. **Streaming `write` (client-streaming).** The destination consumes a `Stream<RecordBatch>` (engine produces) rather than per-batch calls, returning a `WriteOutcome`/acks. *(Phasing allowed: v1 may ship streaming `read` first and keep per-batch `write`, since `write` is the simpler half — but the target is symmetric.)*

4. **Partitions / CDC carry forward, simplified.** Partitioned reads still fan out one configured-instance stream per `Partition` ([[WEIR-T-0027]]). CDC becomes a **long-lived change stream** ([[WEIR-T-0028]]'s `pg_logical_slot_get_changes` polling collapses into a streamed source) — `Checkpoint` messages carry the LSN in `StreamState.opaque`.

5. **Mechanics.** Streaming requires fidius `buffer = PluginAllocated`. Contract version bumps **v0 → v1**; weir is pre-1.0 and the contract is **unshipped**, so we **replace, not dual-path** (no back-compat burden).

## Alternatives Analysis **[CONDITIONAL: Complex Decision]**

| Option | Pros | Cons | Risk | Cost |
|--------|------|------|------|------|
| **A. Streaming read + configured instances (this ADR)** | Realizes A-0014's intent; real backpressure/bounded memory; config+client once; CDC as a natural stream; matches fidius's grain; simpler codegen | Breaking contract rewrite (contract+codegen+engine+runtime+connectors); ties to fidius 0.5+ | Medium | L |
| **B. Streaming read, keep config-per-call** | Smaller change; no construct lifecycle | Re-marshals config + reopens client each stream; misses the "open once" win; configured instances are the natural pair | Low | M |
| **C. Status quo (chunked-pull, v0)** | Works today; zero churn | Manual `has_more` loop; no backpressure/cancel; CDC polls; doesn't use 0.5; keeps a documented *workaround* as the design | Low | None |
| **D. Stream raw batches, engine infers checkpoints** | Slightly simpler message type | Loses connector-owned checkpoint granularity (proven Airbyte model); harder partial-progress semantics | Medium | M |

## Rationale **[REQUIRED]**

Option **A**. fidius 0.5.0 was designed around precisely this shape (REST connector, configured, `read()`-returns-stream), so weir's contract and its execution substrate align instead of fighting. It delivers what [[WEIR-A-0014]] intended before the 0.3.0 request/response workaround — true backpressure and **bounded memory on unbounded sources**, one config marshal + one client open, and CDC as a first-class change stream rather than a polling loop. Configured instances pair naturally with streaming (you bind config once, then stream), so doing only streaming (B) leaves half the win on the table. The decisive enabler is timing: weir is **pre-1.0 with an unshipped contract**, so a breaking v0→v1 change costs us a rewrite we control and no external migration — the cheapest this evolution will ever be.

## Consequences **[REQUIRED]**

### Positive
- True streaming: backpressure + drop-to-cancel + **bounded memory** on unbounded/large sources.
- Config + client/connection opened **once** per run, not re-marshaled per chunk; cleaner `ReadContext`/`WriteContext`.
- CDC is a natural live stream; codegen loses the `has_more` loop and the manual chunk plumbing.
- Contract matches fidius's grain (less impedance, less glue); reverse-ETL writes can stream.

### Negative
- **Breaking contract change**: contract crate (`weir-connector-types`/`weir-connector`), codegen (`weir-codegen`), engine streaming drive loop, `weir-runtime` seam, and **every connector** (echo/faulty/arrow-sink/postgres) rewritten to v1.
- Hard dependency on **fidius 0.5+** (itself a breaking ABI bump, 400→500).
- Checkpoint-on-stream needs care under at-least-once ([[WEIR-A-0011]]): engine commits on `Checkpoint` messages; on cancel/crash, resume from the last committed checkpoint (same guarantee, new mechanism). A mid-stream `ConnectorError` ends the stream → retry from last checkpoint.

### Neutral
- `spec`/`check`/`discover` stay typed unary calls.
- Partition planning ([[WEIR-A-0012]]) + CDC/state semantics unchanged in meaning; only the carrier (a stream of messages) changes.

## Review Schedule **[CONDITIONAL: Temporary Decision]**

Permanent once ratified. **Prerequisites before implementation:** (1) fidius 0.5.0 on crates.io; (2) weir bumped 0.4.2 → 0.5.0 with the breaking recompile verified ([[WEIR-I-0005]]). Implementation is a **follow-on initiative** (contract crate → codegen → engine stream loop → connectors → conformance); this ADR decides only the shape.
