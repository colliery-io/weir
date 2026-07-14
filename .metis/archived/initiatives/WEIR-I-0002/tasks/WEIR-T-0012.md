---
id: airbyte-declarative-manifest-weir
level: task
title: "Airbyte declarative manifest -> weir manifest importer"
short_code: "WEIR-T-0012"
created_at: 2026-06-19T02:12:53.637073+00:00
updated_at: 2026-06-19T02:20:06.069513+00:00
parent: WEIR-I-0002
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0002
---

# Airbyte declarative manifest -> weir manifest importer

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0002]]

## Objective **[REQUIRED]**

Map an **Airbyte low-code (declarative) manifest** (YAML) onto a `weir_manifest::Manifest` ([[WEIR-A-0020]] / [[WEIR-A-0003]]) so the existing codegen ([[WEIR-T-0003]]) lowers it to a `fidius` connector. New `weir-importer` crate: serde structs for the supported subset + a `to_weir_manifest()` mapper. The defining "Migratable Core" capability.

### Scope (v0 subset)
`DeclarativeSource` → `streams[]`/`DeclarativeStream` with a `SimpleRetriever`:
- `HttpRequester` `url_base`+`path` → `base_url`+stream `path`; `BearerAuthenticator`/`ApiKeyAuthenticator` → `Auth` (env-var refs).
- `DpathExtractor.field_path` → `record_selector` (first segment, v0).
- `DefaultPaginator`: `PageIncrement`→`Pagination::Page`, `OffsetIncrement`→`Pagination::Offset`.
- `DatetimeBasedCursor` → `Incremental { cursor_field, cursor_param }`.
- `primary_key` → `primary_key`; `InlineSchemaLoader` json-schema `properties` → `Field`s (json-schema type → `ArrowType`).
Unknown fields ignored (serde); unsupported constructs surface a clear error.

## Backlog Item Details **[CONDITIONAL: Backlog Item]**

{Delete this section when task is assigned to an initiative}

### Type
- [ ] Bug - Production issue that needs fixing
- [ ] Feature - New functionality or enhancement
- [ ] Tech Debt - Code improvement or refactoring
- [ ] Chore - Maintenance or setup work

### Priority
- [ ] P0 - Critical (blocks users/revenue)
- [ ] P1 - High (important for user experience)
- [ ] P2 - Medium (nice to have)
- [ ] P3 - Low (when time permits)

### Impact Assessment **[CONDITIONAL: Bug]**
- **Affected Users**: {Number/percentage of users affected}
- **Reproduction Steps**:
  1. {Step 1}
  2. {Step 2}
  3. {Step 3}
- **Expected vs Actual**: {What should happen vs what happens}

### Business Justification **[CONDITIONAL: Feature]**
- **User Value**: {Why users need this}
- **Business Value**: {Impact on metrics/revenue}
- **Effort Estimate**: {Rough size - S/M/L/XL}

### Technical Debt Impact **[CONDITIONAL: Tech Debt]**
- **Current Problems**: {What's difficult/slow/buggy now}
- **Benefits of Fixing**: {What improves after refactoring}
- **Risk Assessment**: {Risks of not addressing this}

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria **[REQUIRED]**

- [x] `weir-importer` crate parses the Airbyte declarative-manifest subset (serde, unknown fields ignored) and maps it to a valid `weir_manifest::Manifest`.
- [x] Requester (url_base/path/auth), record selector, paginator (page/offset), incremental cursor, primary key, and inline json-schema → Arrow fields all map.
- [x] Unsupported constructs return a clear `ImportError`; output round-trips through `Manifest::from_yaml` (guaranteed valid).
- [x] Tests: a sample Airbyte YAML maps to the expected `Manifest` (base_url, auth env-ref, path, pk, pagination, cursor, schema types + nullability); no-streams errors.

## Status Updates

- **2026-06-19 — DONE.** `weir-importer`: Airbyte low-code subset → `weir_manifest::Manifest`. serde internally-tagged enums + `#[serde(other)]` degrade unknown construct types gracefully; `primary_key` untagged string|array|nested; json-schema type arrays (`["null","string"]`) drive nullability; `{{ config['x'] }}` → `X` env-ref (best-effort). base_url taken from the first stream (one base_url per weir manifest). 2 tests green.

## Test Cases **[CONDITIONAL: Testing Task]**

{Delete unless this is a testing task}

### Test Case 1: {Test Case Name}
- **Test ID**: TC-001
- **Preconditions**: {What must be true before testing}
- **Steps**:
  1. {Step 1}
  2. {Step 2}
  3. {Step 3}
- **Expected Results**: {What should happen}
- **Actual Results**: {To be filled during execution}
- **Status**: {Pass/Fail/Blocked}

### Test Case 2: {Test Case Name}
- **Test ID**: TC-002
- **Preconditions**: {What must be true before testing}
- **Steps**:
  1. {Step 1}
  2. {Step 2}
- **Expected Results**: {What should happen}
- **Actual Results**: {To be filled during execution}
- **Status**: {Pass/Fail/Blocked}

## Documentation Sections **[CONDITIONAL: Documentation Task]**

{Delete unless this is a documentation task}

### User Guide Content
- **Feature Description**: {What this feature does and why it's useful}
- **Prerequisites**: {What users need before using this feature}
- **Step-by-Step Instructions**:
  1. {Step 1 with screenshots/examples}
  2. {Step 2 with screenshots/examples}
  3. {Step 3 with screenshots/examples}

### Troubleshooting Guide
- **Common Issue 1**: {Problem description and solution}
- **Common Issue 2**: {Problem description and solution}
- **Error Messages**: {List of error messages and what they mean}

### API Documentation **[CONDITIONAL: API Documentation]**
- **Endpoint**: {API endpoint description}
- **Parameters**: {Required and optional parameters}
- **Example Request**: {Code example}
- **Example Response**: {Expected response format}

## Implementation Notes **[CONDITIONAL: Technical Task]**

{Keep for technical tasks, delete for non-technical. Technical details, approach, or important considerations}

### Technical Approach
{How this will be implemented}

### Dependencies
{Other tasks or systems this depends on}

### Risk Considerations
{Technical risks and mitigation strategies}

## Status Updates **[REQUIRED]**

*To be added during implementation*
