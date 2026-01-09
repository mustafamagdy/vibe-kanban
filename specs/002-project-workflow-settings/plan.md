# Implementation Plan: Project-Level Workflow Settings

**Branch**: `002-project-workflow-settings` | **Date**: 2026-01-09 | **Spec**: [link](spec.md)
**Input**: Feature specification from `/specs/002-project-workflow-settings/spec.md`

## Summary

Add per-project workflow configuration to enable customizable task status transitions. Extends the existing expandable workflow phases (Testing, AI Review, Human Review) with project-level settings for Human Review toggle, Testing phase behavior, and AI Review iteration limits. Uses JSON column in projects table for configuration storage, new API endpoints for read/write operations, and updates status transition validation to respect project configuration.

## Technical Context

**Language/Version**: Rust 1.75+, TypeScript 5.x, React 18
**Primary Dependencies**: SQLx, actix-web, ts-rs, Tailwind CSS
**Storage**: SQLite with SQLx migrations - projects table to add workflow_config JSON column
**Testing**: cargo test, pnpm run check, pnpm run frontend:dev
**Target Platform**: Linux server, Web browser
**Project Type**: Full-stack web application (Rust backend + React frontend)
**Performance Goals**: Sub-200ms API responses, instant UI updates
**Constraints**: Must maintain backward compatibility with existing projects
**Scale/Scope**: 10-100 concurrent users, single project workspace

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Strict Structure Adherence | ✅ PASS | All code follows `crates/` and `frontend/src/` patterns |
| II. DRY Principle | ✅ PASS | Configuration centralized, shared types via ts-rs |
| III. Code Maintainability | ✅ PASS | Small functions, existing patterns reused |
| IV. Backend Coding Standards | ✅ PASS | rustfmt, unit tests with #[cfg(test)] |
| V. Frontend Coding Standards | ✅ PASS | ESLint + Prettier, PascalCase components |
| VI. Shared Types Management | ✅ PASS | Types generated via ts-rs from Rust |

**Post-Design Re-check**: ALL PRINCIPLES STILL PASS

## Project Structure

### Documentation (this feature)

```text
specs/002-project-workflow-settings/
├── plan.md              # This file (/speckit.plan command output)
├── research.md          # Phase 0 output (research findings)
├── data-model.md        # Phase 1 output (entity definitions)
├── quickstart.md        # Phase 1 output (integration scenarios)
├── contracts/           # Phase 1 output (API specifications)
└── tasks.md             # Phase 2 output (/speckit.tasks command)
```

### Source Code (repository root)

```text
crates/
├── db/
│   ├── src/
│   │   └── models/
│   │       └── project.rs         # Add workflow_config field, update Project model
│   └── migrations/
│       └── *_add_workflow_config.sql  # New migration for JSON column
├── services/
│   └── src/
│       └── services/
│           ├── container.rs       # Update validate_status_transition to use project config
│           └── config/
│               └── versions/
│                   └── v9.rs      # Existing WorkflowConfig struct (reused)
server/
└── src/
    └── routes/
        └── projects.rs            # Add workflow config endpoints
frontend/
└── src/
    ├── pages/
    │   └── settings/
    │       └── WorkflowSettings.tsx  # New workflow settings UI
    └── hooks/
        └── useProjectWorkflowConfig.ts  # New hook for config management
shared/
└── types.ts                    # Generated via pnpm run generate-types
```

**Structure Decision**: Extends existing project structure. New workflow_config JSON column added to projects table. Status transition validation updated to accept project-level WorkflowConfig. Frontend adds new WorkflowSettings component to project settings page.

## Phase 0: Research

**Completed**: This feature extends existing expandable workflow phases (01-09). Key research already done:

- Existing TaskStatus enum in `crates/db/src/models/task.rs`
- Existing WorkflowConfig struct in `crates/services/src/services/config/versions/v9.rs`
- Existing status transition validation in `crates/services/src/services/container.rs`
- Existing project model in `crates/db/src/models/project.rs`
- Existing frontend settings patterns in `frontend/src/pages/settings/`

**Additional Research Needed**:
- SQLite JSON column support and query patterns for workflow_config
- Best practices for storing flexible configuration in relational databases

## Phase 1: Design & Contracts

### Data Model

**Project** (updated):

```rust
// Add to existing Project struct in project.rs
#[serde(default)]
pub workflow_config: Option<ProjectWorkflowConfig>,  // JSON column
```

**ProjectWorkflowConfig** (reuse from config/versions/v9.rs):

```rust
pub struct ProjectWorkflowConfig {
    pub enable_human_review: bool,           // default: false
    pub max_ai_review_iterations: u32,       // default: 3
    pub testing_requires_manual_exit: bool,  // default: true
    pub auto_start_ai_review: bool,          // default: true
    pub ai_review_prompt_template: Option<String>,
}
```

**Migration** (SQLite):

```sql
ALTER TABLE projects ADD COLUMN workflow_config TEXT DEFAULT NULL;
-- SQLite doesn't have native JSON, store as JSON string
-- Use serde_json to serialize/deserialize
```

### API Contracts

**GET /projects/:id/workflow-config**

Response:
```json
{
  "data": {
    "enable_human_review": false,
    "max_ai_review_iterations": 3,
    "testing_requires_manual_exit": true,
    "auto_start_ai_review": true,
    "ai_review_prompt_template": null
  }
}
```

**PATCH /projects/:id/workflow-config**

Request:
```json
{
  "enable_human_review": true,
  "max_ai_review_iterations": 5
}
```

Response:
```json
{
  "data": {
    "enable_human_review": true,
    "max_ai_review_iterations": 5,
    "testing_requires_manual_exit": true,
    "auto_start_ai_review": true,
    "ai_review_prompt_template": null
  }
}
```

### Status Transition Logic (Updated)

```
Configuration-aware transitions:

Human Review disabled:
  Todo → InProgress → Testing → InReview → Done
                                      ↑
                                  (no HumanReview)

Human Review enabled:
  Todo → InProgress → Testing → InReview → HumanReview → Done
                                                  ↑
                                              (optional)

Testing bypass allowed (testing_requires_manual_exit = false):
  InProgress → Testing (optional) → InReview
              OR
  InProgress → InReview (bypass Testing)
```

### Quick Start

1. **New project setup**: Projects created after this feature get default workflow_config
2. **Existing projects**: First access to project settings initializes default workflow_config
3. **Configuration update**: Changes take effect immediately for all tasks

## Complexity Tracking

> No constitution violations detected. Feature uses existing patterns and structures.

## Key Implementation Decisions

| Decision | Rationale |
|----------|-----------|
| JSON column for workflow_config | SQLite doesn't have native JSON; JSON text storage provides flexibility for future config fields |
| Reuse WorkflowConfig from v9 | Already defined struct with all needed fields; no duplication |
| Immediate config apply | Clarified during spec: config changes affect all tasks immediately |
| 0 iterations = validation error | Prevents misconfiguration; must be >= 1 |
