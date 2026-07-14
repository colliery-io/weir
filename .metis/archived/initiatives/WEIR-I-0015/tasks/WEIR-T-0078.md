---
id: c-demo-beat-warehouse-hubspot
level: task
title: "C: Demo beat — warehouse → HubSpot reverse-ETL through the onboarding flow"
short_code: "WEIR-T-0078"
created_at: 2026-07-05T01:26:12.363393+00:00
updated_at: 2026-07-05T01:49:00.392147+00:00
parent: WEIR-I-0015
blocked_by: [WEIR-T-0076]
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0015
---

# C: Demo beat — warehouse → HubSpot reverse-ETL through the onboarding flow

## Parent Initiative

[[WEIR-I-0015]] slice C. Makes reverse-ETL **showable** — the other half of the product thesis
("ingestion AND reverse-ETL") in the founder walkthrough.

## Objective

Add a reverse-ETL beat to the demo script: onboard a SaaS **destination** and activate warehouse
(Postgres) data into it — **HubSpot** — through the real UI/onboarding flow ([[WEIR-T-0076]]/
[[WEIR-T-0077]]), closing the loop with the ingestion beats.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [ ] A new demo-script section: onboard the **HubSpot destination**, create a connection (source =
  the `rates`/`characters` Postgres table already written in the ingestion beats, dest = HubSpot),
  run, and show contacts upserted — the DO/SAY/SHOW format of the existing script.
- [ ] Honest about the **auth requirement** (a HubSpot private-app token) and the **without-a-key**
  fallback (show the onboarding + config; a mock/sandbox note), mirroring how §5 (Auth) handles
  keyless demoing.
- [ ] Frames it as a **headline open-core capability** — a first-class Salesforce
  destination; warehouse→SaaS is the same no-code flow as ingestion.
- [ ] The steps match the **shipped** build (verified against the actual onboarding path from
  [[WEIR-T-0076]]/[[WEIR-T-0077]]); troubleshooting for the common failure (missing token / dest not
  onboarded).
- [ ] Update the roadmap/architecture close so reverse-ETL reads as **shipped**, not future.

## Technical Notes

- Doc-only — but must reflect the **real** flow, so land it after [[WEIR-T-0076]] (and ideally
  [[WEIR-T-0077]]) so the clicks/commands are accurate.
- Reuse the demo's existing Postgres tables as the "warehouse" source (Postgres has a Source role).
- If a live HubSpot key isn't available, the beat can run against a sandbox or be framed as "here's
  the flow" (like the existing optional Auth beat) — don't fabricate a live result.

## Dependencies

- **Blocked by [[WEIR-T-0076]]** (the flow must exist); reads best after [[WEIR-T-0077]] (UI).

## Status Updates

### 2026-07-05 — reverse-ETL demo beat added

New **§4e "Reverse-ETL — activate the warehouse into a SaaS"** in the demo script (DO/SAY/SHOW): onboard the
HubSpot **destination** (now in "Add a connector", marked `· destination`) → connection (source = the
`characters` Postgres table from §4 as the warehouse, dest = HubSpot) → run → contacts upserted.

- **Framed as a headline open-core capability** — a first-class Salesforce destination.
- **Accurate:** token supplied per-connection as **`api_key`** (verified against `from_auth_config`'s
  bearer arm — *not* `HUBSPOT_TOKEN`, only the manifest env hint), injected host-side; `field_map` flagged
  illustrative (align to the read record shape). The **without-a-key** path (demo up to Run) is the reliable
  in-room route; the mapped live upsert is what the mock-HubSpot / mock-Salesforce wire tests cover.
- **§6 close updated:** "reverse-ETL is symmetric" architecture point; roadmap lists reverse-ETL
  destinations as **shipped**. Intro → ~14 min. Troubleshooting for dest-not-onboarded / missing-token /
  staging.

All ACs met. **Complete.**
