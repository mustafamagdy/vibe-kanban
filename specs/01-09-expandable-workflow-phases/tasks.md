---
description: Task list template for feature implementation
---

# Tasks: Expandable Workflow Phases

**Input**: Design documents from `/specs/01-09-expandable-workflow-phases/`
**Prerequisites**: plan.md (required), spec.md (required for user stories), data-model.md, contracts/

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- **Rust backend**: `crates/db/`, `crates/services/`
- **Frontend**: `frontend/src/`
- **Shared**: `shared/types.ts` (generated)

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Database migration and configuration foundation

- [x] T001 Create SQLx migration for workflow statuses in `crates/db/migrations/`
- [x] T002 Create Config v9 with WorkflowConfig in `crates/services/src/services/config/versions/v9.rs`
- [x] T003 Update config/mod.rs to add v9 migration path in `crates/services/src/services/config/mod.rs`
- [x] T004 Add database indexes for status queries per data-model.md

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that MUST be complete before ANY user story can be implemented

**CRITICAL**: No user story work can begin until this phase is complete

- [ ] T005 Update TaskStatus enum in `crates/db/src/models/task.rs` - add Testing and HumanReview variants
- [ ] T006 Add status transition validation function in `crates/services/src/services/container.rs`
- [ ] T007 Create complete_testing method in `crates/services/src/services/container.rs`
- [ ] T008 Create trigger_ai_self_review method in `crates/services/src/services/container.rs`
- [ ] T009 Create handle_ai_review_result method in `crates/services/src/services/container.rs`
- [ ] T010 Create approve_human_review method in `crates/services/src/services/container.rs`
- [ ] T011 Create reject_human_review method in `crates/services/src/services/container.rs`
- [ ] T012 Add unit tests for status transitions in `crates/services/src/services/container.rs` using `#[cfg(test)]`

**Checkpoint**: Foundation ready - user story implementation can now begin in parallel

---

## Phase 3: User Story 1 - Standard Workflow (Priority: P1) 🎯 MVP

**Goal**: Add Testing phase between InProgress and AI Review

**Independent Test**: User drags task to Testing column, task appears there, user exits Testing to AI Review

### Backend - User Story 1

- [ ] T020 [P] [US1] Add PATCH /tasks/:id/status endpoint for testing status in `crates/server/src/routes/tasks.rs`
- [ ] T021 [P] [US1] Add POST /tasks/:id/testing/complete endpoint in `crates/server/src/routes/tasks.rs`
- [ ] T022 [US1] Update finalize_task method to route to Testing instead of Done in `crates/services/src/services/container.rs`

### Frontend - User Story 1

- [ ] T030 [P] [US1] Add 'testing' to TASK_STATUSES array in `frontend/src/pages/ProjectTasks.tsx`
- [ ] T031 [P] [US1] Add 'humanreview' to TASK_STATUSES array in `frontend/src/pages/ProjectTasks.tsx`
- [ ] T032 [US1] Add status label for 'testing' in `frontend/src/utils/statusLabels.ts`
- [ ] T033 [US1] Add status label for 'humanreview' in `frontend/src/utils/statusLabels.ts`
- [ ] T034 [US1] Add Testing column to kanban board in `frontend/src/pages/ProjectTasks.tsx`
- [ ] T035 [US1] Update drag-and-drop to support Testing status transitions in `frontend/src/pages/ProjectTasks.tsx`

**Checkpoint**: User Story 1 complete - tasks can enter Testing and exit to AI Review

---

## Phase 4: User Story 2 - AI Self-Review (Priority: P1)

**Goal**: Automatic AI review after Testing, with subtask generation on failure

**Independent Test**: Task in AI Review generates follow-up, AI provides feedback converted to subtasks if issues found

### Backend - User Story 2

- [ ] T050 [P] [US2] Add POST /tasks/:id/ai-review/result endpoint in `crates/server/src/routes/tasks.rs`
- [ ] T051 [P] [US2] Create create_review_feedback_subtasks method in `crates/services/src/services/container.rs`
- [ ] T052 [US2] Implement iteration limit check in `crates/services/src/services/container.rs`
- [ ] T053 [US2] Update WebSocket event emission for workflow actions in `crates/services/src/services/container.rs`

### Frontend - User Story 2

- [ ] T060 [P] [US2] Add AI Review column to kanban board in `frontend/src/pages/ProjectTasks.tsx`
- [ ] T061 [US2] Add status indicator for AI Review with iteration count in `frontend/src/pages/ProjectTasks.tsx`
- [ ] T062 [US2] Handle task.status.changed WebSocket event for AI Review in `frontend/src/pages/ProjectTasks.tsx`

**Checkpoint**: User Story 2 complete - AI review triggers and handles pass/fail/intervention

---

## Phase 5: User Story 3 - Optional Human Review (Priority: P2)

**Goal**: Human approval gate between AI Review and Done when enabled

**Independent Test**: Admin enables Human Review, tasks appear in Human Review column, reviewer approves to Done

### Backend - User Story 3

- [ ] T080 [P] [US3] Add POST /tasks/:id/human-review/approve endpoint in `crates/server/src/routes/tasks.rs`
- [ ] T081 [P] [US3] Add POST /tasks/:id/human-review/reject endpoint in `crates/server/src/routes/tasks.rs`
- [ ] T082 [US3] Add human review approval tracking fields in `crates/db/src/models/task.rs`

### Frontend - User Story 3

- [ ] T090 [P] [US3] Add Human Review column to kanban board (conditionally rendered) in `frontend/src/pages/ProjectTasks.tsx`
- [ ] T091 [US3] Add approve/reject buttons for Human Review tasks in `frontend/src/pages/ProjectTasks.tsx`
- [ ] T092 [US3] Add reject reason dialog in `frontend/src/components/dialogs/RejectReasonDialog.tsx`

**Checkpoint**: User Story 3 complete - Human Review column and approval flow working

---

## Phase 6: User Story 4 - Workflow Configuration (Priority: P2)

**Goal**: Admin settings UI for workflow behavior

**Independent Test**: Admin modifies settings, changes apply immediately

### Backend - User Story 4

- [ ] T110 [P] [US4] Add GET /projects/:id/workflow-config endpoint in `crates/server/src/routes/projects.rs`
- [ ] T111 [P] [US4] Add PATCH /projects/:id/workflow-config endpoint in `crates/server/src/routes/projects.rs`
- [ ] T112 [US4] Add PUT /projects/:id/workflow-config/reset endpoint in `crates/server/src/routes/projects.rs`

### Frontend - User Story 4

- [ ] T120 [P] [US4] Create WorkflowSettings component in `frontend/src/pages/settings/WorkflowSettings.tsx`
- [ ] T121 [P] [US4] Add Human Review toggle in `frontend/src/pages/settings/WorkflowSettings.tsx`
- [ ] T122 [P] [US4] Add max AI iterations input in `frontend/src/pages/settings/WorkflowSettings.tsx`
- [ ] T123 [P] [US4] Add testing manual exit toggle in `frontend/src/pages/settings/WorkflowSettings.tsx`
- [ ] T124 [US4] Integrate WorkflowSettings into GeneralSettings in `frontend/src/pages/settings/GeneralSettings.tsx`

### i18n - User Story 4

- [ ] T130 [US4] Add workflow settings translations in `frontend/src/i18n/locales/en/settings.json`

**Checkpoint**: User Story 4 complete - Configuration UI fully functional

---

## Phase 7: User Story 5 - Testing Phase Controls (Priority: P3)

**Goal**: Manual vs automatic exit from Testing phase

**Independent Test**: User configures testing exit mode, behavior changes accordingly

### Backend - User Story 5

- [ ] T150 [P] [US5] Add testing_requires_manual_exit check in complete_testing in `crates/services/src/services/container.rs`
- [ ] T151 [US5] Add auto-exit logic when manual exit disabled in `crates/services/src/services/container.rs`

### Frontend - User Story 5

- [ ] T160 [P] [US5] Add testing exit mode toggle in workflow settings in `frontend/src/pages/settings/WorkflowSettings.tsx`
- [ ] T161 [US5] Update Testing column UI based on exit mode in `frontend/src/pages/ProjectTasks.tsx`
- [ ] T162 [US5] Add auto-exit indicator when applicable in `frontend/src/pages/ProjectTasks.tsx`

**Checkpoint**: User Story 5 complete - Testing phase controls operational

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: Improvements that affect multiple user stories

- [ ] T200 [P] Generate TypeScript types via `pnpm run generate-types`
- [ ] T201 [P] Run `pnpm run check` on frontend and fix any type errors
- [ ] T202 [P] Run `cargo check` and `cargo test --workspace` on backend
- [ ] T203 Add integration test for complete workflow in `crates/services/src/services/container.rs`
- [ ] T204 Update documentation in `docs/workflow.md`
- [ ] T205 Test rollback scenario with migration revert

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **User Stories (Phases 3-7)**: All depend on Foundational phase completion
  - User stories can proceed in parallel after Foundational
  - Story order: US1 → US2 → US3 → US4 → US5 (by priority)
- **Polish (Final Phase)**: Depends on all desired user stories being complete

### User Story Dependencies

- **US1 (Standard Workflow)**: Can start after Foundational - No dependencies on other stories
- **US2 (AI Self-Review)**: Can start after Foundational - May integrate with US1 but should be independently testable
- **US3 (Human Review)**: Can start after Foundational - May integrate with US2 but should be independently testable
- **US4 (Configuration)**: Can start after Foundational - Can be tested independently
- **US5 (Testing Controls)**: Can start after Foundational - Depends on US4 settings integration

### Within Each User Story

- Backend endpoints before frontend integration
- Core logic before edge cases
- Story complete before moving to next priority

### Parallel Opportunities

- All Setup tasks marked [P] can run in parallel
- All Foundational tasks marked [P] can run in parallel
- Once Foundational is done, all user stories can start in parallel
- Backend and frontend for same story can run in parallel

---

## Parallel Example: User Story 1

```bash
# Launch backend tasks together:
Task: "Add PATCH /tasks/:id/status endpoint for testing status"
Task: "Add POST /tasks/:id/testing/complete endpoint"

# Launch frontend tasks together:
Task: "Add 'testing' to TASK_STATUSES array"
Task: "Add status label for 'testing'"
Task: "Add Testing column to kanban board"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL - blocks all stories)
3. Complete Phase 3: User Story 1
4. **STOP and VALIDATE**: Test standard workflow independently
5. Deploy/demo if ready

### Incremental Delivery

1. Complete Setup + Foundational → Foundation ready
2. Add User Story 1 → Test independently → Deploy/Demo (MVP!)
3. Add User Story 2 → Test independently → Deploy/Demo
4. Add User Story 3 → Test independently → Deploy/Demo
5. Add User Story 4 → Test independently → Deploy/Demo
6. Add User Story 5 → Test independently → Deploy/Demo
7. Each story adds value without breaking previous stories

### Parallel Team Strategy

With multiple developers:

1. Team completes Setup + Foundational together
2. Once Foundational is done:
   - Developer A: User Story 1
   - Developer B: User Story 2
   - Developer C: User Story 3
3. Stories complete and integrate independently

---

## Task Summary

| Phase | Tasks | Description |
|-------|-------|-------------|
| Phase 1: Setup | 4 | Database migration, Config v9 |
| Phase 2: Foundational | 8 | TaskStatus enum, status transitions |
| Phase 3: US1 | 6 | Standard Workflow |
| Phase 4: US2 | 6 | AI Self-Review |
| Phase 5: US3 | 5 | Optional Human Review |
| Phase 6: US4 | 7 | Workflow Configuration |
| Phase 7: US5 | 4 | Testing Phase Controls |
| Phase 8: Polish | 6 | Cross-cutting concerns |
| **Total** | **46** | |

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- Each user story should be independently completable and testable
- Verify tests fail before implementing
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
- Avoid: vague tasks, same file conflicts, cross-story dependencies that break independence
