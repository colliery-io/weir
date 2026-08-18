---
id: no-code-ui-completeness-express
level: initiative
title: "No-code UI completeness — express and maintain every pipeline the API supports"
short_code: "WEIR-I-0045"
created_at: 2026-08-18T01:57:42.013151+00:00
updated_at: 2026-08-18T02:04:33.557837+00:00
parent: WEIR-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/design"


exit_criteria_met: false
estimated_complexity: M
initiative_id: no-code-ui-completeness-express
---

# No-code UI completeness — express and maintain every pipeline the API supports Initiative

## Context **[REQUIRED]**

The 2026-08-16 alpha review found the no-code promise thins out fast: the connection form sends only name/source/dest/stream/shared-config/every_secs/execution_mode with cron hardcoded to None (`weir-ui/src/main.rs:147-158`), while the API's ConnectionDto supports sync_mode, write_mode, business_keys, cursor_field, cron, and per-side source_config/dest_config (`crates/weir-api/src/lib.rs:57-93`). Consequences: incremental sync and replay-safe upsert — the only crash-safe write mode — are unreachable without curl; destination credentials go into a raw shared JSON textarea; there is no edit flow despite the panel title (editing = retyping everything including secrets); stream discovery reads config untracked so credentialed sources leave the dropdown empty forever; and the tenant-switched admin Setup silently 404s because the UI rewrites routes the server never mirrored (`weir-api/src/lib.rs:198-216`). The e2e suite never exercises the core journey (create-through-form → run → rows arrive). The whole UI is one 1511-line `main.rs`.

## Goals & Non-Goals **[REQUIRED]**

**Goals:**
- A non-engineer can build every alpha-relevant pipeline shape in the UI: incremental sync, upsert with business keys, cursor field, cron or interval, per-side schema-driven config forms.
- Connections are editable without retyping secrets (masked-unchanged sentinel; real round-trip once [[WEIR-I-0047]] lands redact-on-read).
- Stream discovery re-runs when config changes, so credentialed sources actually populate the dropdown.
- Tenant-switched admin Setup works (server mirrors the missing tenant-scoped routes).
- The e2e suite proves the honest journey set: create → run → rows-arrive, resident start/stop, edit, non-admin experience, tenant-switched Setup.
- `main.rs` is split into modules before the form surface grows.

**Non-Goals:**
- Error surfacing ([[WEIR-T-0167]]) and creation-time validation ([[WEIR-T-0166]]) — owned by [[WEIR-I-0042]].
- URL routing/deep links and the localStorage→cookie auth migration — post-alpha polish (tracked, not gating).
- A visual pipeline builder or connector-authoring UI — different product surface, different initiative.
- Per-run log/dead-letter drill-down beyond what [[WEIR-I-0044]] task 6's API provides.

## Detailed Design **[REQUIRED]**

**Open design questions:**

1. **Form scope: progressive disclosure vs full DTO.** Recommendation: the core form adds sync_mode, write_mode (business_keys appear when upsert), cursor_field (offered from the discovered/captured schema when available), and an every/cron toggle; per-side schema-driven config forms replace the shared textarea; raw JSON survives as a collapsible advanced override per side. Do not expose every DTO field — health thresholds etc. stay API-side.
2. **Edit-flow secret semantics.** Until [[WEIR-I-0047]] redact-on-read lands: load the connection, render secret fields masked-unchanged, and on save send a write-only sentinel meaning "preserve stored value". After I-0047: the sentinel becomes the server contract. Decide the sentinel form with I-0047 (one owner for the convention).
3. **Module split shape.** Recommendation: split before the form work — models/fetch/views/{operations,health,platform,setup}/components — mechanical, no behavior change, reviewed as its own commit.
4. **The honest e2e set.** create-through-form → run → rows-arrive (frankfurter → arrow-sink); resident start/stop through the UI; edit-preserves-secret; non-admin (no Platform tab, chip not switcher); tenant-switched Setup smoke. Workers=1 constraint stands until the e2e server moves off single-SQLite.

### Candidate decomposition

| # | Task | Effort | Notes |
|---|---|---|---|
| 1 | Split main.rs into modules (no behavior change) | days | lands first |
| 2 | Per-side schema-driven config forms (source + destination) + advanced JSON override | days | kills the shared-textarea trap |
| 3 | Sync/write/business-keys/cursor/cron form surface | week | after 1-2 |
| 4 | Edit flow with masked-secret sentinel | week | convention shared with [[WEIR-I-0047]] |
| 5 | Server mirrors tenant-scoped Setup routes (catalog import/preview, spec/discover, POST/DELETE connections, schema accept) | days | fixes the silent 404 class |
| 6 | Discovery re-runs on config change (tracked reactive read) | days | with a debounce |
| 7 | Run feed: timestamps + pagination + per-run detail; show committed cursor/chunks | days | consumes [[WEIR-I-0044]] task 6 API |
| 8 | e2e journey suite (the honest set above) | week | gates the initiative |

## Alternatives Considered **[REQUIRED]**

- **Expose the full DTO in the form** — rejected: overwhelms the no-code audience and freezes API details into UI contract during the v0-unstable window ([[WEIR-A-0006]]); progressive disclosure + advanced JSON override covers the same ground.
- **Defer the module split until after alpha** — rejected: the form surface roughly doubles the Setup view; growing a 1511-line file first makes every later change worse. The split is mechanical and cheap now.
- **Build edit on full round-trip of secrets (no sentinel)** — rejected: echoing secrets to make editing easy is exactly the exposure [[WEIR-I-0047]] removes.

## Implementation Plan **[REQUIRED]**

Order: 1 → 2 → 3 → (4 with the I-0047 sentinel convention) ; 5 and 6 independent, any time; 7 after [[WEIR-I-0044]] task 6; 8 accumulates as features land and gates completion. Alpha cut: 1-6 + 8; 7 alpha-should. Design system: existing Aurora components ([[WEIR-A-0035]]); no new visual language.

Dependencies: [[WEIR-I-0047]] (sentinel/redaction convention for task 4), [[WEIR-I-0044]] task 6 (per-run API for task 7), [[WEIR-T-0166]]/[[WEIR-T-0167]] (validation + error surfacing land first so new form errors are visible).

## Exit Criteria

- [ ] A non-engineer builds an incremental, upsert, cron-scheduled pipeline with different source/dest credentials entirely in the UI
- [ ] Editing a connection preserves untouched secret fields (test-proven)
- [ ] An admin switched to another tenant can onboard a connector and create a connection without silent failures
- [ ] The e2e journey suite (create→run→rows, resident start/stop, edit, non-admin) is green in CI
