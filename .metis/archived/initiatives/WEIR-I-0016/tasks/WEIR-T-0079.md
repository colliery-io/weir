---
id: leptos-pipeline-shell-migrate-weir
level: task
title: "Leptos pipeline + shell — migrate weir-ui to Leptos 0.8 + colliery-io-aurora"
short_code: "WEIR-T-0079"
created_at: 2026-07-05T15:35:14.634385+00:00
updated_at: 2026-07-05T15:47:39.696350+00:00
parent: WEIR-I-0016
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0016
---

# Leptos pipeline + shell — migrate weir-ui to Leptos 0.8 + colliery-io-aurora

## Parent Initiative

[[WEIR-I-0016]] (load-bearing). Establishes the Leptos toolchain + the app shell; the views
([[WEIR-T-0080]]/[[WEIR-T-0081]]) fill it in.

## Objective

Convert `weir-ui` from Dioxus to **Leptos 0.8 (CSR)** built from **`colliery-io-aurora`**, and prove
the whole embedded-UI pipeline still works with a **minimal shell**: the Aurora `AppShell` + header
(weir wordmark, Operations/Setup tab switch) + `<AuroraStyles/>`, rendering two empty placeholder
panels. `trunk` builds `weir-ui/dist`, `weir-api` embeds it, the server serves it.

## Reference material (READ FIRST)

- **Aurora API + patterns:** `~/Desktop/aurora-dark/rust/aurora-leptos/PATTERNS.md` (pick-by-intent),
  `.../src/components.rs` + `.../src/lib.rs` (exact component names/props), and
  `~/Desktop/aurora-dark/rust/leptos-gallery/src/main.rs` — a **working** Leptos app using every
  component (copy its `main`/mount + `<AuroraStyles/>` setup). Dep: `colliery-io-aurora = "0.1"`
  (crates.io), imported as `use aurora_leptos::...` (lib name). Leptos 0.8, `features = ["csr"]`.
- **What to port:** `weir-ui/src/main.rs` (the current Dioxus app — the feature/behavior source of truth)
  and `weir-ui/index.html` (trunk config + the `<link data-trunk rel="rust">`; drop the hand-rolled
  `<style>` — Aurora ships its own via `<AuroraStyles/>`).
- **Embed:** `weir-api/build.rs` embeds `weir-ui/dist` (rerun-if-changed) — unchanged. `angreal ui build`
  = trunk build; `.angreal/task_ui.py`.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [ ] **Cargo:** `weir-ui/Cargo.toml` drops `dioxus`, adds `leptos = { version = "0.8", features = ["csr"] }`
  + `colliery-io-aurora = "0.1"` (keep `gloo-net`, `gloo-timers`, `serde`, `serde_json`). Edition works
  for a Leptos csr `main` (mount to `#main`).
- [ ] **Shell:** `src/main.rs` is a Leptos app — `mount_to_body`/`mount` a root component rendering
  `<AuroraStyles/>` + the Aurora `AppShell` (or header + body) with the **weir wordmark**, an
  **Operations | Setup** tab toggle (an `RwSignal<View>`), and two empty `Panel`s ("Operations" /
  "Setup") switched by the tab. No data yet.
- [ ] **Build pipeline green:** `angreal ui build` (trunk) succeeds → `weir-ui/dist`; `cargo build -p weir-cli`
  embeds it; the running server serves the Leptos shell (a Playwright smoke: page loads, both tabs toggle).
- [ ] `index.html` keeps the trunk rust link + font/preconnect but **no bespoke `<style>`** (Aurora owns it).
- [ ] clippy clean on `weir-ui` (build target); no attribution trailers.

## Technical Notes

- **CSR mount:** copy the gallery's `main()` (mount pattern). `<AuroraStyles/>` injects the CSS
  (`AURORA_CSS`) at runtime — simplest, one-time flash acceptable for an embedded control plane.
- Keep the models/helpers you'll need soon (`Connection`, `RunRow`, fetch fns) — but this task only needs
  the shell to compile + render; wiring data is [[WEIR-T-0080]]/[[WEIR-T-0081]].
- Leptos reactivity: `RwSignal` for view state; `view!` macro for markup. Don't fight the compiler — mirror
  the gallery's idioms.
- If an Aurora component's props differ from what's assumed, **read `components.rs`** — it's the API truth.

## Dependencies

- Prereq for [[WEIR-T-0080]] / [[WEIR-T-0081]] / [[WEIR-T-0082]]. Governed by [[WEIR-A-0035]].

## Status Updates

### 2026-07-05 — Leptos + Aurora shell up; pipeline proven; screenshot verified

`weir-ui` is now a **Leptos 0.8 (CSR)** app on **`colliery-io-aurora`** — Dioxus gone.
- **Cargo:** `leptos = { version="0.8", features=["csr"] }` + `colliery-io-aurora = "0.1"` (+ gloo-net/
  gloo-timers/serde/serde_json/wasm-bindgen-futures). Dropped `dioxus`.
- **index.html:** stripped to the trunk rust link — no fonts link, no `<style>`; Aurora's `<AuroraStyles/>`
  injects tokens + components + IBM Plex at runtime. `mount_to_body`.
- **Shell** (`src/main.rs`): `App` renders `<AuroraStyles/>` + a header (`≋ weir` wordmark in the Aurora
  gradient via `--aurora-3`, "control plane" `Text`) + an Operations|Setup `SegmentedControl`
  (`RwSignal<String>`), switching two Aurora `Panel`s. ~15 lines of layout CSS for the top-nav frame (Aurora's
  `AppShell` is sidebar-oriented; colors/type still from Aurora tokens).
- **Component API learned from source:** `SegmentedControl(options, value: RwSignal<String>)`,
  `Panel(title, caption, children)`, `Group(justify, top, gap, …)`, `Text(size, dimmed, mono, …)`,
  `Button(variant, size, on_click: Callback<()>)` — read from `aurora-leptos/src/components.rs`.

**Pipeline green end-to-end:** `angreal ui build` (trunk, leptos 0.8.20 + aurora) ✅ → `dist`; `cargo build
-p weir-cli` embeds it; the server serves it. **Playwright:** `shell.spec.ts` passes (loads; Operations↔Setup
toggle); screenshot confirms the Aurora Dark chrome renders (gradient wordmark, segmented tabs, Panel).

**Remaining:** clippy (running) → commit → complete. Views wired next ([[WEIR-T-0080]]/[[WEIR-T-0081]]).
