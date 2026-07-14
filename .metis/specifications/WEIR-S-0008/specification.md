---
id: migration-importer
level: specification
title: "Migration Importer"
short_code: "WEIR-S-0008"
created_at: 2026-06-17T02:06:27.653222+00:00
updated_at: 2026-06-17T02:06:27.653222+00:00
parent: WEIR-I-0001
blocked_by: []
archived: false

tags:
  - "#specification"
  - "#phase/discovery"


exit_criteria_met: false
initiative_id: NULL
---

# Migration Importer

*Component spec under [[WEIR-I-0001]]. Source: component PRD §7.*

## Overview **[REQUIRED]**

The adoption lever — translates and ports Airbyte connectors and configs into native form, tiered per the capabilities catalog ([[WEIR-S-0001]] §F): declarative YAML is translated, Python CDK is codemodded/adapted, and Java/Kotlin database/warehouse connectors are built natively rather than migrated. It exists so adopters inherit the long-tail ecosystem without us inheriting Airbyte's protocol.

## System Context **[CONDITIONAL: System-Level Spec]**

### Actors
- **Adopter / migrating user**: runs migrations of existing Airbyte connectors and configs.

### External Systems
- **Connector Contract & SDK ([[WEIR-S-0006]])**: the translation target.
- **Connector Catalog ([[WEIR-S-0007]])**: receives migrated connectors.
- **Acceptance-test harness (SDK)**: validates translated connectors.

### Boundaries
Inside: translation/codemod/adapter/config-migration, tiering & confidence reporting. Outside: native DB/warehouse connectors (built in §A/§B, not migrated), runtime wire-compatibility (explicitly not done — ADR-0003).

## Requirements **[REQUIRED]**

### Functional Requirements

| ID | Requirement | Rationale |
|----|-------------|-----------|
| REQ-MI-1 | Translate manifest-only / low-code (declarative YAML) connectors into the native format. | The real mechanical-translation win (§F). |
| REQ-MI-2 | Detect and surface custom Python components that require manual porting. | Transparency; no silent breakage. |
| REQ-MI-3 | Provide codemod and adapter scaffolding for Python CDK connectors. | Long-tail porting (§F). |
| REQ-MI-4 | Migrate existing Airbyte connection/config into native config. | Config migration (§F). |
| REQ-MI-5 | Report a per-connector migration tier and confidence. | Sets expectations honestly. |

### Non-Functional Requirements

| ID | Requirement | Rationale |
|----|-------------|-----------|
| NFR-MI-1 | Fidelity: translated connectors pass the connector acceptance-test harness. | Correctness. |
| NFR-MI-2 | Transparency: never silently emit a broken connector; flag gaps explicitly. | Trust. |
| NFR-MI-3 | Idempotent, re-runnable migrations. | Operability. |

## Architecture Framing **[CONDITIONAL: System-Level Spec]**

### Decision Area: Migration translation fidelity & custom-component handling
- **Context**: Manifest translation coverage; how custom-Python-component connectors are detected and handled. **ADR**: WEIR-A-0020.

### Decision Area: Compatibility strategy / contract mapping target
- **ADR**: WEIR-A-0003, WEIR-A-0014.

## Decision Log **[CONDITIONAL: Has ADRs]**

| ADR | Title | Status | Summary |
|-----|-------|--------|---------|
| WEIR-A-0020 | Migration translation fidelity | proposed | Coverage + custom-component handling. |
| WEIR-A-0003 | Airbyte compatibility strategy | proposed | Migration, not wire-compatibility. |
| WEIR-A-0014 | Connector contract design | proposed | The mapping target. |
