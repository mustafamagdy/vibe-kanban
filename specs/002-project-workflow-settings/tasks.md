# Implementation Tasks: Project-Level Workflow Settings

**Feature**: Project-Level Workflow Settings
**Created**: 2026-01-09
**Branch**: `002-project-workflow-settings`

## Overview

This document provides an actionable, dependency-ordered task list for implementing project-level workflow configuration. Tasks are organized by user story to enable independent implementation and testing.

## Dependencies

```
Phase 1 (Setup)
    │
    ▼
Phase 2 (Foundational) ─ Requires Phase 1
    │
    ├───► Phase 3 (US1) ─ Requires Phase 2
    │
    ├───► Phase 4 (US2) ─ Requires Phase 2
    │
    ├───► Phase 5 (US3) ─ Requires Phase 2
    │
    └───► Phase 6 (US4) ─ Requires Phase 3 (GET endpoint)
                            │
                            ▼
                      Phase 7 (Polish)
```

**Critical Path**: Phase 1 → Phase 2 → Phase 3 → Phase 7 (for MVP)
**Parallel Opportunities**: Phases 3, 4, 5, 6 can run in parallel after Phase 2

## Implementation Strategy

**MVP Scope**: Phases 1-4 (User Story 1 & 2) - Core configuration and status validation
**Phase 5-6**: Can be implemented in parallel or deferred based on priority
**Phase 7**: Cross-cutting concerns applied after all user stories

---

## Phase 1: Setup

**Goal**: Database migration and project model extension

**Independent Test**: Migration runs successfully, Project model includes workflow_config field

### Tasks

- [x] T001 Create SQL migration to add workflow_config TEXT column to projects table in `crates/db/migrations/<timestamp>_add_workflow_config.sql`
- [x] T002 [P] Add workflow_config field to Project struct in `crates/db/src/models/project.rs`
- [x] T003 [P] Update Project SQLx queries to include workflow_config column in `crates/db/src/models/project.rs`
- [x] T004 Add update_workflow_config method to Project model in `crates/db/src/models/project.rs`
- [x] T005 Regenerate TypeScript types via pnpm run generate-types

---

## Phase 2: Foundational

**Goal**: Update status transition validation to use project configuration

**Independent Test**: Status transitions respect project workflow config (Human Review disabled = no InReview→HumanReview transition)

### Tasks

- [ ] T010 [P] Export ProjectWorkflowConfig from config module in `crates/services/src/services/config/mod.rs`
- [ ] T011 Add ProjectWorkflowConfig::default() implementation in `crates/services/src/services/config/versions/v9.rs`
- [ ] T012 Update validate_status_transition to check enable_human_review config in `crates/services/src/services/container.rs`
- [ ] T013 [P] Update validate_status_transition to check testing_requires_manual_exit config in `crates/services/src/services/container.rs`
- [ ] T014 Add helper to load project workflow config in `crates/services/src/services/container.rs`
- [ ] T015 Write unit tests for config-aware status transitions in `crates/services/src/services/container.rs`

---

## Phase 3: User Story 1 - Configure Workflow Settings Per Project (Priority: P1)

**Goal**: API endpoints and UI for viewing/modifying workflow configuration

**Independent Test**: Project admin can view and modify workflow settings via API and UI. Settings persist and affect status transitions.

### Backend Tasks

- [ ] T020 Add GET /projects/:id/workflow-config endpoint in `crates/server/src/routes/projects.rs`
- [ ] T021 [P] Add PATCH /projects/:id/workflow-config endpoint in `crates/server/src/routes/projects.rs`
- [ ] T022 Add config validation (max_ai_review_iterations >= 1) in `crates/server/src/routes/projects.rs`
- [ ] T023 Add project admin permission check for PATCH endpoint in `crates/server/src/routes/projects.rs`
- [ ] T024 Write integration tests for workflow config endpoints in `crates/server/src/routes/projects.rs`

### Frontend Tasks

- [ ] T030 Create useProjectWorkflowConfig hook in `frontend/src/hooks/useProjectWorkflowConfig.ts`
- [ ] T031 [P] Create WorkflowSettings component in `frontend/src/pages/settings/WorkflowSettings.tsx`
- [ ] T032 Add workflow settings to project settings page in `frontend/src/pages/settings/ProjectSettings.tsx`
- [ ] T033 Write frontend tests for WorkflowSettings component

---

## Phase 4: User Story 2 - Configure Testing Phase Behavior (Priority: P1)

**Goal**: Status validation uses testing_requires_manual_exit setting

**Independent Test**: InProgress→InReview transition fails when testing_requires_manual_exit=true, succeeds when false

### Tasks

- [ ] T040 Update tasks.rs update_task endpoint to load project config in `crates/server/src/routes/tasks.rs`
- [ ] T041 Pass project config to validate_status_transition in `crates/server/src/routes/tasks.rs`
- [ ] T042 Return proper error message when transition blocked by config in `crates/server/src/routes/tasks.rs`
- [ ] T043 Update frontend kanban to handle transition rejection gracefully in `frontend/src/pages/ProjectTasks.tsx`
- [ ] T044 Write integration test for Testing bypass scenario

---

## Phase 5: User Story 3 - Configure AI Review Settings (Priority: P2)

**Goal**: Track AI Review iterations and enforce limits

**Independent Test**: AI Review iteration count increments, task stays in InReview after hitting limit

### Tasks

- [ ] T050 Update complete_testing to increment ai_review_iterations in `crates/services/src/services/container.rs`
- [ ] T051 Add check_ai_review_limits method in `crates/services/src/services/container.rs`
- [ ] T052 Update validate_status_transition to enforce max iterations in `crates/services/src/services/container.rs`
- [ ] T053 Add iteration count to task status response in `crates/server/src/routes/tasks.rs`
- [ ] T054 Display iteration count in task details UI in `frontend/src/components/tasks/TaskCard.tsx`
- [ ] T055 Write tests for AI Review iteration tracking

---

## Phase 6: User Story 4 - View Workflow Configuration (Priority: P2)

**Goal**: All project members can view workflow settings (read-only)

**Independent Test**: Non-admin project member can view workflow settings but cannot modify

### Tasks

- [ ] T060 Update GET /projects/:id/workflow-config to allow any project member in `crates/server/src/routes/projects.rs`
- [ ] T061 Update WorkflowSettings component to show read-only view for non-admins in `frontend/src/pages/settings/WorkflowSettings.tsx`
- [ ] T062 Add permission-aware UI to WorkflowSettings in `frontend/src/pages/settings/WorkflowSettings.tsx`
- [ ] T063 Write tests for permission-based access control

---

## Phase 7: Polish & Cross-Cutting Concerns

**Goal**: Integration, error handling, and edge cases

### Tasks

- [ ] T070 Add WebSocket event emission for config changes in `crates/services/src/services/container.rs`
- [ ] T071 Handle null workflow_config (existing projects) with defaults in `crates/services/src/services/container.rs`
- [ ] T072 Add config migration for existing projects on first access in `crates/db/src/models/project.rs`
- [ ] T073 Update frontend kanban to conditionally show HumanReview column based on config in `frontend/src/pages/ProjectTasks.tsx`
- [ ] T074 End-to-end test of complete workflow configuration flow
- [ ] T075 Run cargo check --workspace and fix any errors
- [ ] T076 Run pnpm run check and fix any type errors
- [ ] T077 Run cargo test --workspace and ensure all tests pass

---

## Parallel Execution Examples

### Parallel Set A (After Phase 2):
- T020 (GET endpoint) and T030 (useProjectWorkflowConfig hook)
- T021 (PATCH endpoint) and T031 (WorkflowSettings component)

### Parallel Set B (After Phase 3):
- T040 (update_task validation) and T050 (AI Review iteration)
- T051 (check_ai_review_limits) and T060 (permission update)

### Parallel Set C (Frontend):
- T032 (add to settings page) and T033 (tests)
- T043 (transition rejection handling) and T054 (iteration count UI)

---

## Task Summary

| Phase | Tasks | Description |
|-------|-------|-------------|
| Phase 1 | T001-T005 | Setup: Migration, model, types |
| Phase 2 | T010-T015 | Foundational: Status validation |
| Phase 3 | T020-T034 | US1: Config API + UI |
| Phase 4 | T040-T044 | US2: Testing bypass validation |
| Phase 5 | T050-T055 | US3: AI Review iteration tracking |
| Phase 6 | T060-T063 | US4: Read-only view |
| Phase 7 | T070-T077 | Polish: Integration, tests |

**Total Tasks**: 77
**Parallelizable Tasks**: Marked with [P] marker

---

## Quick Reference: File Paths

### Backend
- `crates/db/migrations/<timestamp>_add_workflow_config.sql` - Migration
- `crates/db/src/models/project.rs` - Project model
- `crates/services/src/services/config/mod.rs` - Config exports
- `crates/services/src/services/config/versions/v9.rs` - WorkflowConfig
- `crates/services/src/services/container.rs` - Status validation
- `crates/server/src/routes/projects.rs` - API endpoints
- `crates/server/src/routes/tasks.rs` - Task updates

### Frontend
- `frontend/src/hooks/useProjectWorkflowConfig.ts` - Data hook
- `frontend/src/pages/settings/WorkflowSettings.tsx` - Settings UI
- `frontend/src/pages/settings/ProjectSettings.tsx` - Settings page
- `frontend/src/pages/ProjectTasks.tsx` - Kanban board
- `frontend/src/components/tasks/TaskCard.tsx` - Task details
