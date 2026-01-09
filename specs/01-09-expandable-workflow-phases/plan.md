# Implementation Plan: Expandable Workflow Phases

**Branch**: `01-09-expandable-workflow-phases` | **Date**: 2026-01-09 | **Spec**: [link](spec.md)
**Input**: Feature specification from `/specs/01-09-expandable-workflow-phases/spec.md`

## Summary

Add configurable workflow phases with Testing, AI Self-Review, and optional Human Review to the task management system. The feature introduces two new task statuses (Testing, HumanReview) and extends the existing TaskStatus enum. Configuration v9 adds WorkflowConfig with settings for Human Review toggle, AI iteration limits, and testing phase behavior. Backend status transitions are extended to route through Testing→AI Review→(optional) Human Review→Done. Frontend adds new kanban columns and workflow settings UI.

## Technical Context

**Language/Version**: Rust 1.75+, TypeScript 5.x, React 18
**Primary Dependencies**: SQLx, actix-web, ts-rs, Tailwind CSS
**Storage**: PostgreSQL with SQLx migrations
**Testing**: cargo test, Vitest, pnpm run check
**Target Platform**: Linux server, Web browser
**Project Type**: Full-stack web application (Rust backend + React frontend)
**Performance Goals**: Sub-200ms API responses, instant UI updates
**Constraints**: Must maintain backward compatibility with existing task statuses
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

**Post-Design Re-check**: ✅ ALL PRINCIPLES STILL PASS

## Project Structure

### Documentation (this feature)

```text
specs/01-09-expandable-workflow-phases/
├── plan.md              # This file (/speckit.plan command output)
├── research.md          # Phase 0 output (/speckit.plan command)
├── data-model.md        # Phase 1 output (/speckit.plan command)
├── quickstart.md        # Phase 1 output (/speckit.plan command)
├── contracts/           # Phase 1 output (/speckit.plan command)
│   ├── config.md
│   └── task-status.md
└── tasks.md             # Phase 2 output (/speckit.tasks command)
```

### Source Code (repository root)

```text
crates/
├── db/
│   ├── src/
│   │   └── models/
│   │       └── task.rs          # TaskStatus enum update
│   └── migrations/
│       └── *_add_workflow_statuses.sql  # New migration
├── services/
│   └── src/
│       └── services/
│           ├── config/
│           │   ├── versions/
│           │   │   └── v9.rs    # New Config v9
│           │   └── mod.rs       # v9 migration path
│           └── container.rs     # Status transition logic
frontend/
├── src/
│   ├── pages/
│   │   ├── ProjectTasks.tsx     # Kanban columns
│   │   └── settings/
│   │       └── GeneralSettings.tsx  # Workflow settings
│   ├── utils/
│   │   └── statusLabels.ts      # Status labels
│   └── i18n/
│       └── locales/
│           └── en/
│               └── settings.json    # Translations
shared/
└── types.ts                    # Generated via pnpm run generate-types
```

**Structure Decision**: Using existing project structure with extensions. New status values added to TaskStatus enum, Config v9 created following v8 pattern, frontend extends existing components with new columns and settings.

## Complexity Tracking

> No constitution violations detected. Feature uses existing patterns and structures.

---

## Phase 0: Research

**Prerequisites**: Technical Context filled

### Research Tasks

- [x] **R1**: Review existing TaskStatus enum in `crates/db/src/models/task.rs`
- [x] **R2**: Review Config v8 pattern in `crates/services/src/services/config/versions/v8.rs`
- [x] **R3**: Review frontend status handling in `frontend/src/pages/ProjectTasks.tsx`
- [x] **R4**: Review status transition logic in `crates/services/src/services/container.rs`

### Research Findings

**R1 - TaskStatus Enum**:
- Current statuses: Todo, InProgress, InReview, Done, Cancelled
- Need to add: Testing, HumanReview
- Rename InReview to AI Review (semantic change, no enum rename needed)
- Migrations needed for CHECK constraint

**R2 - Config v8 Pattern**:
- Config v8 is the current version
- Need to create v9 with WorkflowConfig struct
- Migration path: v8 → v9 adds new workflow fields with defaults

**R3 - Frontend Status Handling**:
- TASK_STATUSES array in ProjectTasks.tsx needs new entries
- statusLabels.ts needs new labels for Testing and HumanReview
- Kanban column rendering filters by status

**R4 - Status Transitions**:
- finalize_task() currently routes to Done
- Need to modify to route to Testing first
- AI review trigger already exists (follow-up tasks pattern)

---

## Phase 1: Design & Contracts

**Prerequisites**: research.md complete

### Data Model

**TaskStatus Enum** (updated from existing):

```rust
pub enum TaskStatus {
    Todo,
    InProgress,
    Testing,        // NEW
    InReview,       // Renamed semantically to AI Review
    HumanReview,    // NEW
    Done,
    Cancelled,
}
```

**WorkflowConfig** (new in Config v9):

```rust
pub struct WorkflowConfig {
    pub enable_human_review: bool,           // default: false
    pub max_ai_review_iterations: u32,       // default: 3
    pub testing_requires_manual_exit: bool,  // default: true
    pub auto_start_ai_review: bool,          // default: true
    pub ai_review_prompt_template: Option<String>,
}
```

**Status Transition Logic**:

```
Todo → InProgress → Testing → InReview → HumanReview → Done
                                   ↓
                              (if HumanReview disabled)
                                   ↓
                                  Done

InReview → {pass} → HumanReview/Done
           {fail} → InProgress (with subtasks)
           {max iterations} → Manual Intervention (in InReview)
```

### API Contracts

**Config Settings**:

```yaml
/workflow-config:
  GET: Retrieve current workflow configuration
  PATCH: Update workflow configuration
```

**Task Status Transitions**:

```yaml
/tasks/{id}/status:
  PATCH: Update task status (Testing, InReview, HumanReview, Done)
```

---

## Phase 2: Implementation Planning

**Prerequisites**: data-model.md, contracts/, quickstart.md complete

See `/speckit.tasks` for task generation.

---

## Quick Links

- [Feature Specification](spec.md)
- [Research Findings](research.md)
- [Data Model](data-model.md)
- [API Contracts](contracts/)
- [Tasks](tasks.md) - Generated by /speckit.tasks
