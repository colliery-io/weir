---
id: 001-in-flight-transform-mapping-model
level: adr
title: "In-flight transform/mapping model"
number: 1
short_code: "WEIR-A-0026"
created_at: 2026-06-17T03:00:28.815277+00:00
updated_at: 2026-06-21T22:54:49.244042+00:00
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

# ADR-0026: In-flight transform/mapping model

**Status:** Accepted (ratified 2026-06-21, Dylan Storey). *Raised by: [[WEIR-S-0004]] Sync Engine, [[WEIR-S-0002]] Control Plane; relates to [[WEIR-A-0007]] data model.*

**Partially realized:** the contract already carries `MappingSpec { ops: Vec<MappingOp> }` with
`Select | Drop | Rename` ([[WEIR-A-0029]]); the engine currently passes records through unmapped
(`// passthrough stub`). Ratifying this ADR green-lights building the real engine-owned stage (adding
`cast` / `filter` / computed-field expressions + the row-JSON and Arrow evaluators) — scoped into the
reverse-ETL initiative, where warehouse→SaaS field shaping needs it.

## Context **[REQUIRED]**

The capabilities catalog ([[WEIR-S-0001]] §D) puts **light in-flight mapping** (rename, filter, cast, basic field-shaping) *in* scope, and explicitly puts **heavy transformation/modeling** *out* of scope (dbt's job). The connector contract ([[WEIR-A-0014]]) deliberately excludes transform from connector methods to keep connectors pure extract/load. So a transform/mapping capability must live *somewhere* — this ADR decides where and how, and (critically) how to keep it light.

The risk this ADR exists to manage is **scope creep**: the moment in-flight mapping grows joins, aggregations, or arbitrary code, weir has reinvented dbt and broken its own Out-of-Scope boundary. The decision must encode a defensible boundary, not just a feature.

## Decision **[REQUIRED]**

*Decided:*

1. **Transform is a connection-level stage owned by the Engine**, applied in the record path **between `read` and `write`** — not a connector method, not a warehouse step. It is configured per connection ([[WEIR-A-0007]]: the connection "carries … mapping").
2. **Declarative, not code.** Mapping is expressed as declarative config (consistent with [[WEIR-A-0014]]'s declarative-first stance): an ordered list of operations over fields/records.
3. **Bounded operation set (the scope boundary):** `select`/`drop` (column projection), `rename`, `cast` (type coercion within the logical type system), `filter` (row predicate), and simple computed fields via a **non-Turing-complete expression language** (field refs, literals, comparison/boolean/arithmetic, a fixed allow-list of scalar functions). **Explicitly excluded:** joins, aggregations, grouping, cross-record state, lookups, arbitrary code/UDFs. Those are dbt's job.
4. **Operates on the record stream in both encodings** ([[WEIR-A-0014]] §3): row-JSON for the long tail, Arrow for the bulk path (where projection/filter are cheap, vectorizable column ops).

## Alternatives Analysis **[CONDITIONAL: Complex Decision]**

| Option | Pros | Cons | Verdict |
|--------|------|------|---------|
| Engine-owned declarative stage (chosen) | Connectors stay pure; reusable across any source→dest; declarative; bounded | Needs a small expression language + evaluator | **Chosen** |
| Transform as a connector method | Connector controls shaping | Every connector reimplements mapping; couples transform to connector code | Rejected |
| No in-flight transform at all | Simplest | Loses the catalog's P0/P1 "light mapping" capability; forces dbt for trivial renames/filters | Rejected |
| Full expression/transform engine | Powerful | Becomes dbt; violates Out-of-Scope; unbounded scope | Rejected |

## Rationale **[REQUIRED]**

- Keeping transform in the Engine keeps connectors pure extract/load (clean contract, easier authoring/migration) and makes mapping reusable across every connector pair.
- Declarative + a deliberately crippled expression language is what *enforces* the dbt boundary — you cannot express a join, so you cannot drift into modeling.
- Column projection/filter map naturally onto Arrow for the bulk path, so the light transform is nearly free where throughput matters.

## Consequences **[REQUIRED]**

### Positive
- Trivial renames/filters/casts without standing up dbt; connectors stay pure.
- Boundary is enforced by the grammar, not by policy.

### Negative
- A small expression language + evaluator is net-new (must be implemented for both row and Arrow paths).
- Temptation to extend the grammar will be persistent; the exclusion list must be guarded.

### Neutral
- Lands in the data model ([[WEIR-A-0007]], connection carries the mapping spec) and the record path defined by [[WEIR-A-0014]].

## Review Schedule **[CONDITIONAL: Temporary Decision]**

### To move draft → discussion → decided:
- Define the operation set + expression grammar (v0) and confirm it has **no** join/aggregate/UDF escape.
- Confirm evaluation over both row-JSON and Arrow encodings.
- Cross-reference from [[WEIR-S-0001]] §D and [[WEIR-A-0007]].

### Review trigger
- Revisit if real demand appears for anything beyond projection/filter/cast/compute — but the default answer to "can we add joins/aggregates?" is **no, that's dbt** (escalate to a vision/scope change, not a grammar tweak).
