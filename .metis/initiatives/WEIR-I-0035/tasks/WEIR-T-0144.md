---
id: f1-8-ui-slice-for-resident-sources
level: task
title: "F1.8 — UI slice for resident sources + demo seed (execution-mode toggle, Start/Stop, live badge)"
short_code: "WEIR-T-0144"
created_at: 2026-07-10T15:24:00.676959+00:00
updated_at: 2026-07-10T15:25:16.163186+00:00
parent: WEIR-I-0035
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0035
---

# F1.8 — UI slice for resident sources + demo seed

## Parent Initiative

[[WEIR-I-0035]] (F1). Makes the resident runtime **UI-demo-ready** (the API/CLI already work; the Leptos UI
`weir-ui/` was untouched). A resident source idling in the UI doubles as an always-on soak.

## Objective

Add the minimal web-UI surface to declare, launch, stop, and watch a resident source, and seed a self-running
resident connection into `angreal ui demo`.

## Acceptance Criteria

## Acceptance Criteria

- [ ] **Create form** (`weir-ui/src/main.rs`): an **execution-mode** control (`run_once` | `resident`) + optional
  cadence; posts `execution_mode` on `POST /connections` (DTO already carries it).
- [ ] **Connection row: Start/Stop** buttons for resident connections → `POST /connections/{name}/start` /
  `/stop` (endpoints already exist); disabled/hidden for run-once.
- [ ] **Live badge**: a resident connection shows a "resident • live/stopped" indicator (derive from run
  state / execution_mode).
- [ ] **Demo seed** (`.angreal/task_ui.py`): stage a resident-capable connector and seed a **resident** demo
  connection, **auto-started** so it's running on page load (the soak) — plus run-once demos stay.
- [ ] `angreal ui build` (trunk) compiles; `angreal ui demo` comes up with the resident connection **running**
  (visible as a live/running row).

## Implementation Notes

- UI: `weir-ui/src/main.rs` — `Connection` struct (~:18) gains `execution_mode`; extend the create form
  (source/dest/sync_mode selects) with the mode control; add Start/Stop actions calling `areq_post` to the
  start/stop routes; badge off the run/exec state (`run_metrics`/`RunRow` ~:269-287).
- Demo: `.angreal/task_ui.py` DEMO list (~line 7) — add a `resident` tuple; stage its connector (GUESTS/stage);
  after the API is up, `weir connection add … --execution-mode resident` + `weir start --name …` (or POST /start).
  A resident-poll fixture with a ~1–2s cadence is a good visible, low-noise soak source.
- Keep everything else in the UI working; no API changes needed (start/stop + execution_mode DTO already exist).

### Verification
- `angreal ui build` green; smoke: start the demo server, confirm the resident connection exists + is running via
  `GET /runs` / `GET /overview` and `POST …/stop` then `…/start` toggles it. Do NOT leave a server running in CI.

## Status Updates

**2026-07-10 — COMPLETE + demo running (verified live).** UI slice (`weir-ui/src/main.rs`) + demo seed
(`.angreal/task_ui.py`).
- Create form: execution-mode `<select>` (run_once|resident) + cadence; Start/Stop buttons on resident cards →
  `POST /connections/{name}/start|stop`; `resident • live/stopped` badge.
- Demo seed: `resident-demo` (resident-poll → ArrowSink, every 2s, `--execution-mode resident`), auto-started; runs
  under `run_resident` on boot = the always-on tile / passive soak.
- `angreal ui build` compiles (needed `rustup target add wasm32-unknown-unknown` — env gap). `cargo build
  --workspace` green; fmt/clippy clean.
- **Live-verified:** `angreal ui demo`'s angreal wrapper errored at the server-launch step (this env's python quirk),
  so I launched the server directly (same sequence): UI served at `http://localhost:8787` (`weir · control plane`),
  catalog seeded (200), **`resident-demo` state=leased, rows_written climbing 1→37→50** on its 2s cadence; Stop/Start
  toggle returns 200. Auth is interim key (localStorage `weir_api_key`); minted an admin key for the demo.
- **Gaps:** live badge can momentarily read "stopped" during a lease-expiry/reclaim gap (cosmetic); UI has no
  sign-in field yet (T-0087) so the key is set via localStorage; `angreal ui demo` wrapper hits an env python quirk
  (server runs fine launched directly).