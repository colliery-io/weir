---
id: 001-native-connectors-as-first-class
level: adr
title: "Native connectors as first-class"
number: 1
short_code: "WEIR-A-0016"
created_at: 2026-06-17T02:12:15.229235+00:00
updated_at: 2026-06-22T19:17:00.663453+00:00
decision_date:
decision_maker: Dylan Storey
parent:
archived: false

tags:
  - "#adr"
  - "#phase/superseded"


exit_criteria_met: false
initiative_id: NULL
---

# ADR-0016: Native connectors as first-class

**Status:** ⚠️ **SUPERSEDED by [[WEIR-A-0030]]** (WASM-always, 2026-06-22). The native cdylib path this ADR
made the trusted/first-party default is retired — all connectors are wasm; "first-party/trusted" is now an
origin attribute, not a packaging kind. *(Originally: Decided, determined by [[WEIR-A-0002]]; Dylan Storey,
2026-06-17.) Raised by: [[WEIR-S-0006]].*

## Context **[REQUIRED]**

Python is first-class for authoring ([[WEIR-A-0001]]). The question was whether **native (Rust) connectors** are a first-class, supported authoring path or an internal-only optimization. [[WEIR-A-0002]] settled it: the native **dylib FFI path is the default for trusted/first-party connectors**, including the high-throughput DB/warehouse + Arrow bulk path.

## Decision **[REQUIRED]**

**Yes — native (Rust) connectors are first-class.** The native dylib path ([[WEIR-A-0002]]) is the **default execution path for trusted/first-party connectors** and the home of the bulk Arrow throughput path ([[WEIR-A-0014]] §3). It is not Python-only authoring:

- **Native (Rust) dylib** — first-class; trusted/first-party default; bulk/high-throughput.
- **Python** — first-class too, via the PyO3 boundary (first-party) and via WASM (open, compiled).
- All author against the **same transport-neutral `Connector` contract** ([[WEIR-A-0014]]); the SDK offers both a native and a Python surface.

## Alternatives Analysis **[CONDITIONAL: Complex Decision]**

| Option | Verdict |
|--------|---------|
| Native first-class (chosen) | **Chosen** — performance where data volume lives; required by [[WEIR-A-0002]]'s trusted dylib path |
| Python-only authoring | Rejected — forfeits native performance on the hottest connectors |

## Rationale **[REQUIRED]**

The highest-data-volume connectors (DB/warehouse, CDC, bulk load) justify native performance, and [[WEIR-A-0002]] already makes the native dylib path the trusted default. The transport-neutral contract lets native and Python coexist behind one definition.

## Consequences **[REQUIRED]**

### Positive
- Native throughput on the connectors that move the most data; one contract across native + Python.

### Negative
- The SDK maintains two authoring surfaces (native Rust + Python) against the one contract.

### Neutral
- Interacts with the execution model ([[WEIR-A-0002]]), packaging ([[WEIR-A-0015]]), and contract ([[WEIR-A-0014]]).
