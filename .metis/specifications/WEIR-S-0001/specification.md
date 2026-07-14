---
id: capabilities-catalog
level: specification
title: "Capabilities Catalog"
short_code: "WEIR-S-0001"
created_at: 2026-06-17T02:00:10.604119+00:00
updated_at: 2026-06-17T02:00:10.604119+00:00
parent: WEIR-I-0001
blocked_by: []
archived: false

tags:
  - "#specification"
  - "#phase/discovery"


exit_criteria_met: false
initiative_id: NULL
---

# Capabilities Catalog

*Companion to the weir Vision ([[WEIR-V-0001]]). The second rung of the top-down decomposition: vision (why) → **capabilities (what the platform can do)** → components (the building blocks, WEIR-S-0002…0013) → interfaces/decisions (ADRs). This document is deliberately implementation-neutral — no components, no interfaces, no resolution of open architecture decisions. Where an open decision (notably the execution/isolation model, ADR-0002) is load-bearing, it appears here only as a **capability requirement**, with the mechanism deferred.*

## Overview **[REQUIRED]**

This catalog enumerates what weir can do, organized into capability areas A–M. It bounds the product (see Out of Scope) without prescribing how any capability is delivered.

**Legend.**
- **[Core]** ships in the Apache project. **[Periphery]** is a vendor's proprietary offering. **[Core→Periphery]** has an open baseline with advanced behavior in the periphery.
- **P0** = required for a credible first release. **P1** = fast-follow. **P2** = later or periphery. (Proposed, aligned to the vision's phasing; adjust freely.)
- Differentiators versus parity-with-Airbyte are called out in prose.

## Capability Areas **[REQUIRED]**

### A. Source connectivity & extraction
- **Connect to source categories** — SaaS/REST APIs, relational and NoSQL databases, files and object stores; event/streaming later. [Core] P0
- **Change data capture** from databases (log-based). [Core] P1 — table stakes against Fivetran
- **Schema discovery** — introspect available streams/tables/columns. [Core] P0
- **Sync modes** — full refresh, cursor-based incremental, log-based. [Core] P0/P1
- **Stream and column selection.** [Core] P0
- **Resumable per-stream checkpointing** — resume after failure without a full re-read. [Core] P0 — robustness differentiator
- **Partitioned / parallel reads** for throughput. [Core] P1 — performance differentiator

### B. Destination loading
- **Load to warehouses, lakes, lakehouses, files/object stores.** [Core] P0
- **Write semantics** — append, dedup/upsert, full overwrite. [Core] P0
- **Schema handling** — raw and normalized output; schema-drift/evolution handling. [Core] P0/P1
- **Typed source-to-destination mapping.** [Core] P0

### C. Reverse ETL / activation
*A headline open-core capability — first-class reverse ETL, including the Salesforce destination.*
- **Write modeled data from the warehouse to operational systems** (CRM/SaaS: HubSpot, Salesforce, and others). [Core] P1
- **Upsert/dedup on business keys; field mapping to target objects.** [Core] P1
- **Rate-limit-aware, retry-safe writes** to destination APIs. [Core] P1
- **Idempotent activation** — no-duplicate guarantees on writes. [Core] P1

### D. Pipeline (connection) definition & configuration
- **Define a connection** — source → destination, stream selection, sync mode, schedule, mapping. [Core] P0
- **Light in-flight mapping** — rename, filter, basic field shaping (not heavy transformation; see Out of Scope). Model defined in [[WEIR-A-0026]]. [Core] P0/P1
- **Connection config** — secrets binding, namespaces, naming conventions. [Core] P0
- **Test/validate/dry-run before activation.** [Core] P0 — UX differentiator

### E. Connector authoring & extensibility
- **Connector SDK** for engineers to author connectors. [Core] P0
- **No-code/low-code declarative connector builder.** [Core] P1 — parity (open in Airbyte)
- **AI-assisted connector authoring.** [Core] P1 — differentiator; meets the shift to agent-built pipelines
- **Connector packaging and versioning.** [Core] P0
- **Connector catalog / marketplace** — discover, install, version connectors. [Core] P1
- **Safely execute third-party/untrusted connector code** with isolation. [Core] P0/P1 — *capability requirement; mechanism deferred to components/ADR-0002*

### F. Migration from Airbyte
*Adoption lever — but a tiered one. Airbyte connectors come in four flavors (java, low-code, python, manifest-only); "mechanically translatable" applies to some, not all. The count-weighted and value-weighted migration stories differ.*
- **Translate manifest-only / low-code (declarative YAML) connectors** — the formulaic API-source majority by count — into the native format. [Core] P1 — the real mechanical-translation win
- **Port low-code connectors with embedded custom Python components** — the declarative skeleton translates; the Python escape-hatch classes need adaptation. [Core] P1/P2
- **Codemod + adapter + porting guide for full Python CDK connectors** — complex sources, file-based sources, some destinations. [Core] P2
- **Re-implement, not translate, the Java/Kotlin connectors** — database sources (CDC) and warehouse destinations. Highest-data-volume; built natively as part of the P0 core (§A, §B), not migrated.
- **Connection/config migration** — map existing Airbyte connection configs onto ours. [Core] P1
- *Reality check: migration's real job is the long-tail API sources, where translation genuinely works. The databases and warehouses you most depend on you build first-party regardless.*

### G. Scheduling & orchestration
- **Schedule syncs** (cron/intervals). [Core] P0
- **Manual and API-triggered runs.** [Core] P0
- **Backfills and ranged re-syncs.** [Core] P1
- **Basic inter-sync dependency/sequencing.** [Core] P1
- **Retry policies and failure handling.** [Core] P0
- **Drive from external orchestrators** (Airflow; see L). [Core] P1

### H. Execution, isolation & scaling
- **Reliable sync execution** with retries and durable state. [Core] P0
- **Concurrency control** across many connections. [Core] P0/P1
- **Resource isolation between connectors/tenants.** [Core baseline → Periphery advanced] P1 — *capability requirement; mechanism deferred (ADR-0002)*
- **Worker autoscaling on Kubernetes.** [Core→Periphery] P1 — a headline open-core capability; basic autoscaling open, advanced/managed periphery
- **Lean per-connector run footprint** — a small per-connector footprint (versus a container-per-connector model). [Core] — mechanism deferred (ADR-0002)

### I. Observability, state & lineage
- **Sync status, run history, per-stream progress.** [Core] P0
- **Logs and structured error surfacing.** [Core] P0
- **Metrics** — rows, bytes, latency, freshness. [Core] P1
- **Freshness / SLA tracking.** [Core→Periphery] P1
- **Lineage & metadata emission via OpenLineage.** [Core] P1 — differentiator and integration anchor
- **Alerting** on failures (email/webhook baseline). [Core→Periphery] P1

### J. Security, secrets & governance
- **Secrets management** for connection credentials. [Core] P0
- **Platform authentication** (baseline). [Core] P0
- **RBAC, SSO, audit logging.** [Periphery] P1/P2 — the Airbyte-enterprise set; periphery, but the core must expose clean hooks
- **Multi-tenancy.** [Periphery] P2 — core provides the isolation primitives it needs
- **Encryption in transit/at rest; private networking.** [Core baseline → Periphery advanced]

### K. Deployment & operations
- **Self-hosted single-node deploy** (simple path). [Core] P0
- **Basic Kubernetes deploy** (chart/manifests). [Core] P0/P1
- **Production Helm operator** — lifecycle, upgrades, scaling. [Periphery] P1 — core vendor value
- **Configuration and upgrade management.** [Core baseline → Periphery]
- **Backup/restore of platform state.** [Core] P1

### L. Integration & programmatic surface
- **REST/programmatic API** covering all operations. [Core] P0
- **Terraform/IaC provider.** [Core] P1 — parity
- **Airflow provider/operator.** [Core] P1 — integration differentiator
- **DataHub lineage integration** (via OpenLineage). [Core] P1
- **Superset metadata exposure.** [Core] P2
- **Embeddable library use** (programmatic, PyAirbyte-style). [Core] P2

### M. User experience (no-code)
- **Web UI** to configure sources, destinations, connections. [Core] P0
- **Monitoring/status UI.** [Core] P0
- **Connector builder UI.** [Core] P1
- **Migration flow UI.** [Core] P1/P2
- *Freedom to redesign flows for lower friction (per vision) — a differentiator, not a parity copy.*

## Constraints **[CONDITIONAL: Has Constraints]**

### Out of scope (non-capabilities, to bound the product)
- **Heavy in-warehouse transformation / modeling** — dbt's job; we do light in-flight mapping only.
- **BI, dashboards, visualization** — Superset's job; we feed it.
- **Sub-second true streaming** — pair with Kafka/Flink; our cadence is batch, micro-batch, and CDC.
- **Data catalog / governance platform** — DataHub's job; we emit lineage to it, we do not replace it.
- **General workflow orchestration beyond syncs** — Airflow's job; we integrate, we do not replace.

## Note on deferred decisions

The open architecture decisions from the vision (notably the execution/isolation primitive, ADR-0002) are intentionally *not* resolved here. They surface above only as capability requirements — "safely execute untrusted connectors," "lean per-connector footprint," "resource isolation between tenants." The mechanisms that satisfy them are decided at the components (WEIR-S-0002…0013) and ADR rungs.
