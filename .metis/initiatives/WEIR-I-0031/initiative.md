---
id: connector-contract-codegen-repair
level: initiative
title: "Connector contract codegen — repair + complete the generation seam"
short_code: "WEIR-I-0031"
created_at: 2026-07-08T14:58:56.656895+00:00
updated_at: 2026-07-09T02:53:35.283122+00:00
parent: WEIR-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: M
initiative_id: connector-contract-codegen-repair
---

# Connector contract codegen — repair + complete the generation seam Initiative

## Context

Every full-code connector opens with the *same* `mod weir_guest_types` block — ~143 lines, 35 `WitType`-derived types — copied byte-for-byte (`rest` == `s3` to the byte; `rest`↔`postgres` differ only by a 2-line comment). It has to be inlined because `fidius_build::emit_wit` only reads a guest's own `src/lib.rs`.

**But the abstraction meant to prevent hand-copying is already broken.** `weir-codegen` emits the block as a format-string template whose `RecordBatch` is still `{ Rows, Arrow }` — missing the `Changes` variant the live contract (`weir-connector-types` and the checked-in generated `rest`) has carried since T-0113. The generated `rest` connector is stamped "GENERATED … do not edit by hand," yet contains `Changes` — it was hand-patched after generation. So the seam has silently drifted: some connectors are generated (from a stale template), the rest are hand-copied, and the host copy in `weir-connector-types` is a fourth transcription. Changing the contract today is ~9 synchronized edits with **no compiler enforcement** across the WASM/host boundary — a mis-ordered `WitType` field only fails at runtime on the interface-hash check.

Surfaced by the 2026-07-08 architecture review as a **Strong** candidate — the worst locality in the workspace; the deletion test concentrates strongly (one origin removes seven-plus transcriptions).

## Goals & Non-Goals

**Goals:**
- **One origin** for the guest-side WIT contract; N generated inlines, zero hand-copies.
- The codegen output derived from / pinned to `weir-connector-types` so it cannot drift again — regenerated to include `Changes`.
- The four full-code connectors **and** the wasm-fixtures wired through the generation path.
- A **drift guard** (test/CI) that fails if a guest's contract block diverges from the origin.

**Non-Goals:**
- Changing the contract's *shape* (`RecordBatch` / `ChangeRecord` / field set) — this fixes duplication + staleness, not the contract.
- Retiring self-contained guests ([[WEIR-A-0030]]) — guests stay self-contained; the block is *emitted*, not linked from a shared crate.
- Rewriting fidius or `emit_wit`.

## Detailed Design

*(Grounded against fidius-wit 0.5.5 + the on-disk connectors — for sign-off before decomposition.)*

- **Mechanism (validated by reading the tool).** `fidius_wit::generate_from_path` — what `emit_wit` calls — **follows external `mod m;`** into `src/m.rs` / `src/m/mod.rs`, recursively (generate.rs:101–120). It resolves the module by **`m.ident` only** (ignores `#[path]`) and, being a `syn` text-walk, **does not expand `include!`**. Inline vs external mod produce the *same* `mod_path`, so moving the block out of `lib.rs` **does not change the WitType interface hash** — it's load-compatible with the host.
- **The shape, then:** each guest declares `mod weir_guest_types;` in `lib.rs` and carries a `src/weir_guest_types.rs` file *= the contract block*. `emit_wit` follows it and finds every `WitType`.
- **One canonical block.** A single source owned by `weir-codegen` (a `guest_contract.rs` template / const), **with `Changes` / `ChangeRecord` / `ChangeOp` restored**. `weir-codegen`'s `wasm.rs` stops embedding the ~125-line static block; it emits `mod weir_guest_types;` + writes `src/weir_guest_types.rs` from the canonical.
- **Sync + drift guard.** A generator (a `weir-codegen` entry point, driven by an `angreal connectors sync-contract` task) writes the canonical block into every full-code connector + fixture's `src/weir_guest_types.rs`; a **workspace test asserts each checked-in copy is byte-identical to the canonical** (+ that `Changes` is present) — "kept in sync by CI," not by discipline.

Aligns with **WEIR-A-0014 §2** (the contract as "codegen, not a runtime interpreter") and **WEIR-A-0030** (self-contained guests) — the block is still textually present in each guest's own source tree; it's now *generated from one origin*, not hand-copied.

## Alternatives Considered

- **`include!` a shared file into each guest.** **Ruled out** — `fidius_wit` is a `syn` text-walk; it sees `include!(…)` as a macro call and never expands it, so the `WitType` types would be invisible to `emit_wit`.
- **`#[path = "…/shared.rs"] mod weir_guest_types;` pointing every guest at one file.** **Ruled out** — `fidius_wit` resolves a `mod m;` by `m.ident` (→ `src/m.rs`), ignoring the `#[path]` attribute; it would look for `src/weir_guest_types.rs` and fail.
- **Symlink each `src/weir_guest_types.rs` → one shared file.** Works (`read_to_string` follows symlinks) but fragile across OSes and git configs. Rejected in favour of a generated file + a byte-identical drift guard.
- **A shared crate the guests depend on for the types.** Rejected — the `WitType` derives must be textually present in the guest's own source tree for `emit_wit` to see them.
- **Just hand-fix the stale template.** Fixes today's drift, prevents none of tomorrow's. Rejected.

## Implementation Plan

Proposed decomposition (**2 tasks**) — for sign-off:
1. **Canonical block + codegen + drift guard.** Extract the block into one canonical source (with `Changes`/`ChangeRecord`/`ChangeOp` restored). Rewire `weir-codegen`'s `wasm.rs` to emit `mod weir_guest_types;` + write `src/weir_guest_types.rs` from the canonical (deleting the stale ~125-line static block). Add the workspace drift-guard test. Prove a generated (manifest) guest still builds for `wasm32-wasip2`.
2. **Apply to full-code connectors + fixtures.** Restructure `postgres` / `rest` / `rest-dest` / `s3` + `wasm-fixtures/{echo,slow,faulty,arrow-sink}` to `mod weir_guest_types;` + a generated `src/weir_guest_types.rs`; delete the inline blocks. `angreal test connectors` (the wasm build) + the drift guard + full suite + clippy green.

**Exit criteria:** one canonical block source; every guest (generated + full-code + fixtures) uses `mod weir_guest_types;` sourced from it; the stale-template drift fixed (`Changes` present everywhere); the drift-guard test fails on divergence; wasm builds + full suite + clippy clean.
