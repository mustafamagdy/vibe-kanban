# Feature Specification: Expandable Workflow Phases

**Feature Branch**: `01-09-expandable-workflow-phases`
**Created**: 2026-01-09
**Status**: Draft
**Input**: Add configurable workflow phases with Testing, AI Self-Review, and optional Human Review

## User Scenarios & Testing

### User Story 1 - Standard Workflow (Priority: P1)

As a user working on tasks, I want my tasks to automatically enter a testing phase after coding is complete, so that I can verify the implementation before marking it as done.

**Why this priority**: This is the primary workflow change that all users will experience. It introduces the Testing phase as a standard gate before completion.

**Independent Test**: User drags a task from "In Progress" to "Testing" column. The task appears in Testing column with a manual exit option. User completes testing and moves task to AI Review.

**Acceptance Scenarios**:

1. **Given** a task is in "In Progress" and the user marks it complete, **When** they move it to Testing, **Then** the task status changes to "Testing" and appears in the Testing column.

2. **Given** a task is in "Testing", **When** the user completes testing and moves it forward, **Then** the task transitions to "AI Review" status automatically.

---

### User Story 2 - AI Self-Review (Priority: P1)

As a user, I want AI to review my work after testing, so that I can catch issues before considering the task done.

**Why this priority**: AI self-review is a core part of the new workflow, providing automated quality gates without human intervention.

**Independent Test**: Task in AI Review generates a follow-up task with review instructions. AI provides feedback which is converted to subtasks if issues are found.

**Acceptance Scenarios**:

1. **Given** a task enters "AI Review" status, **When** AI review is triggered, **Then** a follow-up task is created to document the review process.

2. **Given** AI review identifies issues, **When** review fails, **Then** subtasks are created for each issue and the parent task returns to "In Progress".

3. **Given** AI review passes all checks, **When** review completes, **Then** the task moves to "Done" (if Human Review disabled) or "Human Review" (if enabled).

---

### User Story 3 - Optional Human Review (Priority: P2)

As a project manager, I want to optionally require human review before task completion, so that I can ensure quality standards are met for critical work.

**Why this priority**: Provides flexibility for teams that need manual approval gates without forcing it on all teams.

**Independent Test**: Admin enables Human Review in settings. Tasks complete AI Review then appear in Human Review column. Reviewer approves and moves to Done.

**Acceptance Scenarios**:

1. **Given** Human Review is enabled in project settings, **When** a task completes AI Review, **Then** the task enters "Human Review" status instead of Done.

2. **Given** Human Review is disabled in project settings (default), **When** a task completes AI Review, **Then** the task enters "Done" status directly.

3. **Given** a task is in "Human Review", **When** a reviewer approves it, **Then** the task moves to "Done".

---

### User Story 4 - Workflow Configuration (Priority: P2)

As an administrator, I want to configure workflow settings, so that I can tailor the review process to my team's needs.

**Why this priority**: Configuration enables teams to customize their workflow without code changes.

**Independent Test**: Admin opens settings, finds Workflow section, modifies settings like Human Review toggle and max AI iterations, saves changes. New workflow is active immediately.

**Acceptance Scenarios**:

1. **Given** an administrator accesses project settings, **When** they navigate to Workflow section, **Then** they can toggle Human Review on/off.

2. **Given** workflow settings are modified, **When** changes are saved, **Then** new tasks immediately use the updated workflow.

3. **Given** an administrator sets max AI review iterations, **When** AI review fails repeatedly, **Then** the task is marked for manual intervention after reaching the limit.

---

### User Story 5 - Testing Phase Controls (Priority: P3)

As a user, I want control over when testing starts and ends, so that I can properly manage my testing workflow.

**Why this priority**: Optional quality-of-life feature for teams with specific testing requirements.

**Independent Test**: User configures testing to require manual exit. Task enters Testing column. User completes actions and manually exits Testing to trigger AI Review.

**Acceptance Scenarios**:

1. **Given** testing requires manual exit is enabled, **When** a task enters Testing, **Then** it stays in Testing until the user explicitly moves it forward.

2. **Given** testing requires manual exit is disabled, **When** a task enters Testing, **Then** testing auto-exits after completion criteria are met.

---

### Edge Cases

- ~~What happens when an AI review iteration limit is reached without passing?~~ → Resolved: Task stays in AI Review for manual intervention
- How does the system handle existing tasks that were in "In Review" before this feature?
- ~~What happens if Human Review is toggled mid-workflow for an active task?~~ → Resolved: In-flight tasks immediately adopt new workflow setting
- How are tasks with the old status values handled during migration?

## Requirements

### Functional Requirements

- **FR-001**: System MUST support a "Testing" status that appears between "In Progress" and "AI Review" in the workflow.
- **FR-002**: System MUST support a "Human Review" status that appears between "AI Review" and "Done" when enabled.
- **FR-003**: System MUST allow administrators to enable/disable Human Review via project settings; setting applies immediately to all tasks including those already in workflow.
- **FR-004**: System MUST automatically trigger AI review when a task exits Testing status.
- **FR-005**: System MUST create follow-up tasks to document AI review feedback.
- **FR-006**: System MUST revert a task to "In Progress" when AI review fails, creating subtasks for identified issues.
- **FR-007**: System MUST limit AI review iterations based on configured maximum; when limit is reached without passing, task is marked for manual intervention in AI Review status.
- **FR-008**: System MUST allow configuration of testing phase exit mode (manual vs. automatic).
- **FR-009**: System MUST provide workflow configuration options for max AI iterations and testing behavior.
- **FR-010**: System MUST migrate existing "In Review" tasks to the new "AI Review" status on upgrade.
- **FR-011**: System MUST gracefully handle backward compatibility for tasks created before this feature.

### Key Entities

- **Task**: Represents work items that progress through workflow states. Has a status field that determines current phase.
- **WorkflowConfig**: Project-level configuration for workflow behavior including Human Review toggle, AI iteration limits, and testing options.
- **TaskStatus**: Enum representing workflow states (Todo, InProgress, Testing, AI Review, HumanReview, Done, Cancelled).

## Success Criteria

### Measurable Outcomes

- **SC-001**: Users can complete the standard workflow (Todo → InProgress → Testing → AI Review → Done) within a single session without blockers.
- **SC-002**: When Human Review is enabled, 100% of tasks completing AI Review appear in Human Review column for manual approval.
- **SC-003**: AI review feedback is converted to actionable subtasks and users are notified when review completes.
- **SC-004**: Workflow configuration changes take effect immediately for new tasks without requiring application restart.
- **SC-005**: Existing tasks with legacy status values display correctly and can continue through the new workflow.
- **SC-006**: Task status transitions are visible in the kanban board with appropriate column grouping for Testing and Human Review phases.

## Assumptions

- AI review logic and feedback generation already exist in the system and only need to be integrated into the new workflow.
- The kanban board UI can accommodate new columns without significant redesign.
- Configuration migration from previous versions will handle existing project settings.
- Backend status transition logic can be extended without breaking existing workflows.
- Frontend status handling is flexible enough to support the new statuses without major refactoring.

## Clarifications

### Session 2026-01-09

- Q: AI Review Iteration Limit Behavior → A: Task marked for manual intervention, stays in AI Review
- Q: Human Review Toggle Mid-Workflow → A: All in-flight tasks immediately adopt new workflow setting
- Q: AI Review Completion Time Target → A: No time target - Asynchronous only
