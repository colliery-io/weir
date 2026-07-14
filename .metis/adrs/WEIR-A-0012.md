---
id: 001-partition-slice-planning-ownership
level: adr
title: "Partition/slice planning ownership"
number: 1
short_code: "WEIR-A-0012"
created_at: 2026-06-17T02:12:06.315566+00:00
updated_at: 2026-06-21T22:36:39.848267+00:00
decision_date:
decision_maker:
parent:
archived: false

tags:
  - "#adr"
  - "#phase/decided"


exit_criteria_met: false
initiative_id: NULL
---

# ADR-0012: Partition/slice planning ownership

**Status:** Accepted (ratified 2026-06-21). *Raised by: [[WEIR-S-0004]] Sync Engine, [[WEIR-S-0006]] Connector Contract & SDK.*

**Realized by** [[WEIR-T-0027]] (partitioned/parallel reads): the connector declares partitionability via
`PartitionScheme` (`Unpartitioned | ByCursorRange | ByKeyShards | ByParent`) in its `StreamInfo`; the
orchestrator **materializes the slices** (one configured stream + `Partition` per slice) and schedules
them under concurrency limits, each with its own checkpoint. This ADR ratifies what is implemented.

## Context **[REQUIRED]**

Partitioned/parallel reads are a performance differentiator (capabilities §A). Who decides how a stream is sliced — the connector or the Engine?

## Decision **[REQUIRED]**

*Decided:* the **connector declares partitionability** (key ranges, cursors, shards) in the contract; the **Engine plans the slices** and schedules them under concurrency limits. Planning stays central; knowledge of how to partition stays with the connector.

## Alternatives Analysis **[CONDITIONAL: Complex Decision]**

| Option | Pros | Cons | Risk | Cost |
|--------|------|------|------|------|
| Connector declares, Engine plans (chosen) | Central scheduling/fairness; connectors stay simple | Contract must express partitioning | Medium | Medium |
| Connector self-partitions and self-schedules | Connector autonomy | Reliability/fairness logic duplicated per connector | High | High |
| Engine infers partitioning | No connector burden | Engine can't know source specifics | High | Medium |

## Rationale **[REQUIRED]**

Mirrors the engine's central-reliability philosophy: connectors declare capability, the Engine owns when/how-many. Keeps the long tail trivial to author.

## Consequences **[REQUIRED]**

### Positive
- Centralized concurrency/fairness; simple connectors.

### Negative
- The contract ([[WEIR-A-0014]]) must carry a partitioning declaration.

### Neutral
- Interacts with delivery semantics ([[WEIR-A-0011]]).
