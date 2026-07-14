---
id: operations-view-in-leptos-cards
level: task
title: "Operations view in Leptos — cards, run feed, run-detail, live polling"
short_code: "WEIR-T-0080"
created_at: 2026-07-05T15:35:53.237590+00:00
updated_at: 2026-07-05T16:00:00.305484+00:00
parent: WEIR-I-0016
blocked_by: [WEIR-T-0079]
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0016
---

# Operations view in Leptos — cards, run feed, run-detail, live polling

## Parent Initiative

[[WEIR-I-0016]]. Fills the Operations tab of the [[WEIR-T-0079]] shell with Aurora components.

## Objective

Reproduce the **Operations view** from the current Dioxus UI (`weir-ui/src/main.rs`) in Leptos +
Aurora: the connection cards, the run feed, the run-detail overlay, and live polling — feature parity,
no behavior loss.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [ ] **Connection cards** — one per `/connections` entry: name, `source → dest` flow, latest run **state**
  (Aurora `Pill`/`HealthPill`, colored by `status_color`), a **Run** button (`POST /connections/{n}/run`),
  metrics (rows/duration from the latest `RunRow`), and delete (`DELETE /connections/{n}`). Aurora
  `SimpleGrid` of `Panel`s (or the card widget).
- [ ] **Run feed** — the `/runs` list (id, connection, state, metrics), most-recent first; rows clickable
  to open the run detail.
- [ ] **Run-detail overlay** — Aurora `Modal` (open-state `RwSignal<bool>`): the run's **logs**
  (`/connections/{c}/logs`) + **dead-letters** (`/connections/{c}/dead-letters`), styled by level/severity.
- [ ] **Live polling** — connections + runs refetch on an interval (`gloo-timers` + a Leptos resource /
  effect), so the feed updates without a manual refresh; header live-stats (active/rows/runs) reflect it.
- [ ] Parity check: nothing the Dioxus Operations view did is lost (compare against `main.rs`).
- [ ] `angreal ui build` green; clippy clean.

## Reference

- Port from `weir-ui/src/main.rs` (Operations rsx + `ConnectionCard` component + `fetch_connections`/
  `fetch_runs`/`fetch_logs`/`fetch_dead_letters` + `run_metrics`/`fmt_dur`/`latest_state`).
- Aurora components: see [[WEIR-T-0079]]'s reference list (`PATTERNS.md`, `components.rs`, `widgets.rs`,
  the gallery). Status color via `aurora_leptos`'s `status_color(&str)`; don't hardcode hex.

## Dependencies

- **Blocked by [[WEIR-T-0079]]** (shell + toolchain).

## Status Updates

### 2026-07-05 — Operations view in Leptos + Aurora; parity + verified

Ported the Operations view to Leptos + Aurora components (models/fetch/helpers re-derived from the old
Dioxus `main.rs`):
- **Cards** — a `SimpleGrid cols=3` of connection cards (custom `.weir-card` layout, Aurora tokens): name,
  `source ──▶ dest` flow, a state `Pill` colored by a weir→token map (`done`→OK, `leased`→ICE, `failed`→BAD,
  `pending`→VIOLET), `stream · …`, latest-run metric (`#id · N rows · dur`), and `del`/`Run` `Button`s (with
  `stop_propagation` so they don't open the detail).
- **Run feed** — an Aurora `Table` (mono) of `/runs` (id / connection / state `Pill` / detail), rows click
  to open the detail.
- **Run-detail** — Aurora `Modal` (`RwSignal<bool>`): connection name + dead-letters + logs, fetched into
  signals on open (`spawn_local`).
- **Live polling** — a `spawn_local` loop refetches `/connections` + `/runs` every 800ms; header shows
  live `N runs` / `N rows`. Run/delete via `POST /connections/{n}/run` / `DELETE /connections/{n}` with a toast.

**Learned:** Leptos `Callback` is `Copy` (pass to many cards); `Button.on_click` is `Option<Callback<()>>`;
`Modal` is conditional (renders only when `open`); precompute row fields before `view!` (partial-move).

**Verified:** `angreal ui build` ✅; clippy clean; Playwright `shell.spec.ts` + `operations.spec.ts` (card →
modal with Dead-letters/Logs) pass; **screenshot** shows the seeded `fx-demo` card (`done` pill,
`rest ──▶ arrow-sink`, `165 rows · 2.6s`) + the feed. Feature parity with the Dioxus view. **Complete.**
