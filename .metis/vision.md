---
id: weir
level: vision
title: "weir"
short_code: "WEIR-V-0001"
created_at: 2026-06-17T01:48:16.277130+00:00
updated_at: 2026-06-17T01:48:16.277130+00:00
archived: false

tags:
  - "#vision"
  - "#phase/draft"


exit_criteria_met: false
initiative_id: NULL
---

# weir Vision

**An open-source, no-code data ingestion & reverse-ETL platform — Apache-2.0, built to be donated to the Apache Software Foundation.**

*Status: Draft v0.2 (ported from the Sluice vision doc). This is a vision and strategy document, not a spec. It exists to ratify the bets we are making so the capabilities, component specs, and ADRs that follow are built on agreed ground.*

## Purpose **[REQUIRED]**

There is no fully open-source, foundation-governed, no-code data ingestion and reverse-ETL platform. The category is dominated by proprietary SaaS (Fivetran, Estuary), and the platform that defined the open space — Airbyte — is open-core: the platform, connector protocol, SDK, and no-code builder are open, while some operationally valuable pieces (RBAC, SSO, audit, multitenancy, premium connectors) ship in a separate commercial edition. weir sets out to build a fully-open counterpart — in effect an Apache-2.0 clone of that capability set — and donate it to the Apache Software Foundation.

weir exists to build the *complete* product, license all of it under Apache 2.0, and donate it to the Apache Software Foundation so that it cannot later be enclosed by anyone — including us. The business does not come from owning the core; it comes from operating it best.

SkaleData is the project's sponsor and its first commercial vendor — funding the build through Colliery's technical work — not the owner of the project and not merely a distribution that bundles it. The project itself is vendor-neutral and ASF-governed; the vendor's advantage is first-mover position, the proprietary operational periphery, and engineering expertise. The central argument: the moat and the donation are not in tension — the extension interface that lets the community contribute *is* the same interface that lets a vendor sell, and the discipline of donating to the ASF is what forces that interface to be clean.

## Product/Solution Overview **[CONDITIONAL: Product/Solution Vision]**

A self-hostable platform that moves data both directions:

- **Ingestion (EL):** from APIs, databases (including CDC), files, and SaaS sources into warehouses, lakes, and lakehouses.
- **Reverse ETL (data activation):** from the warehouse back out to operational systems (HubSpot, Salesforce, and other CRMs/SaaS), treated as a first-class sync type rather than a bolted-on destination.
- **No-code surface:** a web UI for configuring connections, plus a no-code/low-code connector builder, so non-engineers can stand up pipelines.
- **A connector SDK** for engineers to author connectors, designed for AI-assisted authoring from day one — the connector ecosystem has shifted hard toward agent-generated pipelines, and we meet authors where they now are.

**Target audience:** organizations wanting a self-hosted, fully open ingestion *plane* — and the vendors and consultancies who will operate it for them.

## Current State **[REQUIRED]**

Airbyte is open-core, and the boundary is part of the product structure. The platform core, connector protocol, Python SDK, no-code builder, and connector marketplace are open. Some capabilities used to run at scale — RBAC, audit logging, SSO, multitenancy, and private networking — ship in Self-Managed Enterprise. Reverse ETL illustrates the split: the Salesforce destination is a commercial-edition connector, while HubSpot is in the open edition. weir's aim is to provide the whole capability set in one fully-open, ASF-governed distribution.

The rest of the field confirms the gap rather than filling it:

- **Fivetran / Estuary** are fully proprietary — the price ceiling we push against, not alternatives.
- **dlt** is genuinely open (Apache 2.0) but is a code-first Python *library*, not a no-code platform; its commercial platform is closed. A complement and ecosystem ally, not a competitor for the same slot.
- **Snowflake Openflow** is GUI-based ingestion on Apache NiFi, but bound to Snowflake.

## Future State **[REQUIRED]**

weir occupies the precise unoccupied position: **a no-code *platform* (UI, scheduling, connector catalog, deployment) that is complete and fully open, with reverse-ETL as a first-class capability and Kubernetes autoscaling in the open core — extensible enough that a company can build a proprietary periphery around it without crippling the center.**

We pursue three strategic framings, **sequenced** rather than chosen between:

1. **Migration-grade compatibility (not drop-in).** We own our own connector format; we do not run Airbyte connectors unmodified. We guarantee a *trivial migration path* instead.
2. **Capability parity, freedom on flows.** We match Airbyte's platform capabilities while remaining free to design the UX flows our own way.
3. **A day-one roadmap of design goals:** a lean core, autoscaling in the open, first-class reverse ETL, and a clean protocol designed from scratch.

## Major Features **[CONDITIONAL: Product Vision]**

- **Bidirectional sync engine** — EL ingestion and reverse-ETL activation as one symmetric primitive.
- **Clean, owned connector contract** — typed schemas, robust state/checkpoint semantics, partitioned/parallel reads, structured error taxonomy, native reverse-ETL semantics.
- **Connector SDK + low-code builder + AI-assisted authoring** — meeting the shift to agent-built pipelines.
- **Airbyte migration importer** — mechanical translation of declarative connectors; codemod + adapter for Python CDK.
- **First-class reverse ETL** — HubSpot and Salesforce connectors in the open core.
- **Kubernetes autoscaling in the open core** — a headline open-core capability.
- **OpenLineage emission + integration surface** — Airflow provider, DataHub, Superset, Terraform.
- **Lean per-connector runtime** — a WASM-based runtime with a small per-connector resource footprint.

## Business Requirements Overview **[CONDITIONAL: Business Vision]**

The open-core boundary is stated as a **moat**, and the ASF makes it explicit and unforgiving: an Apache project ships everything under Apache 2.0 and is vendor-neutral.

| Layer | Lives where | License |
|---|---|---|
| Connector contract, SDK & builder, catalog, sync engine / plugin runtime, scheduler, web UI, state store, extension interfaces, migration importer, single-node + basic K8s deploy, OpenLineage emission, reverse-ETL primitives incl. HubSpot/Salesforce | **Apache project** | Apache 2.0 |
| Production Helm operator + autoscaler, managed/multi-tenant control plane, SSO/RBAC/audit/governance, premium & SLA-backed connectors, polished vendor-native integrations, support | **Vendor (Colliery / SkaleData)**, separate repos | Vendor's choice |

The moat is **operational, not functional**: a vendor competes on running it well, never on withholding features the core needs to work. The strongest honest answer to "why this vendor" is expertise — they wrote the patterns everyone else reverse-engineers.

## Success Criteria **[REQUIRED]**

- The open core is a product someone would happily run in production **for free**, with nothing purchased.
- A credible Airbyte replacement for a real pipeline, with a **one-command migration story** for declarative connectors.
- First-class reverse ETL (HubSpot + Salesforce) shipping in the open core.
- Basic Kubernetes worker autoscaling in the open core.
- The project enters the ASF as an incubator podling with clean IP provenance, and is on a credible path to graduation (vendor-neutral governance, a *diverse* external contributor community).

## Principles **[REQUIRED]**

1. **The open core stands alone.** Genuinely complete and production-usable with nothing purchased. We never cripple the center to sell the periphery. This is both an ethical commitment and a requirement of ASF graduation.
2. **The moat is operational, not functional.** Compete on running it well — autoscaling, the Helm operator, governance, managed control plane, SLA-backed connectors, expertise.
3. **Interfaces before implementation.** Every extension point (connectors, plugins, integrations, deployment) is a designed, documented, versioned interface — simultaneously the community contribution surface, the moat boundary, and the integration surface. We design these first.
4. **IP-clean from commit one.** Develop in the open, permissive dependencies only, clear contributor agreements, proper LICENSE/NOTICE hygiene, so ASF IP clearance is uneventful.
5. **Vendor-neutral by construction.** The brand, governance, and roadmap must be able to belong to a community. SkaleData is sponsor and first vendor — a participant, not the owner.

## Architecture Bets (interface-first)

High-level bets; each becomes its own component spec or ADR before implementation.

- **Language split:** **Rust** for the control plane (scheduler, sync engine, worker orchestration, autoscaler) — small static binary, predictable resource use, natural K8s-operator fit; **Python** for the connector SDK and builder, because the connector ecosystem (Airbyte CDK, Singer, dlt, Meltano) and AI-assisted authoring are Python. *(ADR-0001)*
- **Connector-execution model (headline open decision):** connectors run as **orchestrated services** against a stable contract; the open question is the *execution/isolation primitive* along a leanness↔isolation spectrum — in-process ABI (leanest, weakest isolation), bare/OS-sandboxed subprocess, WASM component (lean *and* capability-isolated), or container (heaviest, demoted to a compatibility escape hatch). The cloacina (embedded orchestration) and fidius (macro-generated FFI) patterns are reference expertise, not consumed dependencies. *(ADR-0002)*
- **Protocol:** own a clean connector contract; ship a migration importer — translation tooling, not runtime wire-compatibility. *(ADR-0003)*
- **Reverse ETL** as a first-class, symmetric primitive: a sync is source→destination where a destination may be a warehouse *or* an operational system.
- **Integration surface:** Airflow provider, OpenLineage emission (DataHub), Superset metadata, Terraform — adoption drivers that lean open.

## Governance & the Path to Apache

The donation shapes the work from the first commit, not at the end.

- **Entry:** the project enters the ASF as an *incubator podling*, sponsored after an Incubator PMC vote, with a formal proposal.
- **IP clearance:** every copyright holder signs a CLA or Software Grant (Corporate CLA / Software Grant from Colliery; Individual CLAs from each human contributor). Getting this right *before* accumulating code is far cheaper than retrofitting.
- **Naming & trademark:** the ASF must own the marks before graduation; the name cannot be a Colliery or SkaleData brand.
- **Graduation risk — single-vendor dynamics:** the ASF cares deeply about vendor neutrality and a *diverse* contributor community. One funded contractor plus a sponsor company is a known graduation hazard. Community-building is a first-class workstream — develop in the open, recruit outside committers, make connector authoring genuinely easy.

## Phasing (vision-level; not a roadmap)

- **Phase 0 — Foundations & design.** Ratify this vision. Write the capabilities catalog, component specs, and ADRs (language boundary, connector contract, execution model, migration scope). Stand up the repo with IP hygiene and contributor agreements. Run formal name clearance.
- **Phase 1 — Migratable core.** Rust core running the native connector contract, basic scheduler, web UI, single-node + basic K8s deploy, plus the migration importer for declarative connectors.
- **Phase 2 — Friction fixes & reverse ETL.** Full connector contract, first-class reverse ETL (HubSpot + Salesforce), connector builder, OpenLineage emission.
- **Phase 3 — Operate-it-well periphery (parallel, vendor repos).** Helm operator, autoscaler, Airflow provider, vendor integrations; begin the open core's autoscaling story.
- **Phase 4 — Community & donation.** Open development in earnest, recruit external committers, prepare the incubation proposal, begin IP clearance.

## Risks

- **Single-vendor graduation risk** — answered by the community workstream and a trivially easy contribution surface (connectors).
- **Open-core credibility risk** ("they'll enclose it later") — answered structurally: the core is at the ASF and *cannot* be enclosed. Our single strongest differentiator; lead with it.
- **Ecosystem cold-start** — answered by the migration importer inheriting the long-tail catalog through translation.
- **Connector-execution model unproven at scale** — answered by treating it as an explicit, deferred decision (ADR-0002) with proven reference patterns.
- **Scope sprawl** — a Fivetran-class product is enormous; phasing keeps each phase shippable.
- **Moat erosion** (someone forks and offers the periphery cheaper) — answered by operational excellence and expertise.

## Constraints **[REQUIRED]**

**Out of scope (to bound the product):**
- Heavy in-warehouse transformation / modeling — dbt's job; we do light in-flight mapping only.
- BI, dashboards, visualization — Superset's job; we feed it.
- Sub-second true stream *computation* — windowing, joins, aggregation over a stream — pair with Kafka/Flink. Our compute cadence stays batch, micro-batch, CDC. **Amended 2026-07-08 (Signal Fabric enablement):** *long-lived resident sources that stay up and emit* (continuous-polling / event-reader) and *socket fan-out to many subscribers* are now **in scope** — weir may **carry** a live signal continuously and fan it out, but still **never computes over it or decides on it**. "Stay up and emit," not "compute over a stream." Tracked in initiatives [[WEIR-I-0035]]–[[WEIR-I-0040]] (F1–F6).
- Data catalog / governance platform — DataHub's job; we emit lineage to it.
- General workflow orchestration beyond syncs — Airflow's job; we integrate.

**Hard constraints:**
- Everything in the open core ships under **Apache 2.0**; no vendor-only dependencies in the core.
- ASF vendor-neutrality and IP-provenance requirements bind from commit one.

## Decision Log (ratify before decomposition)

- **Name.** *Adopted:* **weir** as the project name (replacing the earlier "Sluice" working codename). The name must still clear a formal trademark/collision check (crate/PyPI/registered-mark, .org domain) before it can be locked for ASF graduation. Crate `weir`, PyPI `weir-py`, and repo `colliery-io/weir` are reserved.
- **Language split.** Rust core + Python connector SDK. *Recommendation: yes; pressure-test team Rust depth for the hot path.* → ADR-0001
- **Compatibility strategy.** Migration, not runtime compatibility. *Recommendation: yes.* → ADR-0003
- **Integration line.** Which Airflow/DataHub/Superset integrations are open vs. vendor. *Recommendation: integrations lean open as adoption drivers; managed/multi-tenant experience is the vendor's.*
- **First deliverable.** Component specs + connector-contract design before a big PRD (design-first). *Recommendation: yes.* → ADR-0014
- **Connector-execution model (OPEN — headline).** Execution/isolation primitive along the leanness↔isolation spectrum. *Recommendation: resolve in the architecture spec / ADR-0002.*
