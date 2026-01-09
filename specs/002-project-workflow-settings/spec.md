# Feature Specification: Project-Level Workflow Settings

**Feature Branch**: `002-project-workflow-settings`
**Created**: 2026-01-09
**Status**: Draft
**Input**: Extend expandable workflow phases with per-project configuration settings for workflow behavior

## Clarifications

### Session 2026-01-09

- Q: Configuration changes for in-flight tasks → A: Immediate apply - Configuration changes take effect immediately for all tasks regardless of current status.
- Q: AI Review iterations = 0 → A: Validation error - Configuration is rejected as invalid, must be >= 1

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Configure Workflow Settings Per Project (Priority: P1)

As a project administrator, I want to configure workflow behavior for my project so that I can customize how tasks move through phases based on my team's needs.

**Why this priority**: Core configuration functionality that enables all other workflow customization. Without this, the workflow phases are rigid and cannot adapt to different team workflows.

**Independent Test**: Project admin accesses project settings, modifies workflow options, saves changes. New tasks follow the updated workflow rules. Configuration persists across sessions.

**Acceptance Scenarios**:

1. **Given** a project exists, **When** the project admin accesses project settings, **Then** they can view and modify workflow configuration options including Human Review toggle, Testing phase behavior, and AI Review iterations.

2. **Given** workflow settings are modified, **When** the admin saves changes, **Then** the configuration persists and applies to all new task status transitions.

3. **Given** Human Review is disabled in project settings, **When** a task completes AI Review, **Then** it transitions directly to Done (skipping Human Review phase).

4. **Given** Human Review is enabled in project settings, **When** a task completes AI Review, **Then** it transitions to Human Review for manual approval before Done.

---

### User Story 2 - Configure Testing Phase Behavior (Priority: P1)

As a project administrator, I want to control whether the Testing phase requires manual exit or can be bypassed so that I can enforce quality gates or allow faster workflows.

**Why this priority**: Testing phase behavior directly impacts team velocity and quality standards. Different teams have different needs - some require mandatory testing, others prefer faster iteration.

**Independent Test**: Admin enables/disables "Testing requires manual exit" setting. Tasks in Testing column behave accordingly - either requiring explicit action to exit or allowing automatic progression.

**Acceptance Scenarios**:

1. **Given** "Testing requires manual exit" is enabled, **When** a task is in Testing status, **Then** users must explicitly move it to another status (InReview or Done).

2. **Given** "Testing requires manual exit" is disabled, **When** a task execution completes, **Then** it can optionally bypass Testing and proceed directly to AI Review.

3. **Given** Testing phase is configured, **When** tasks enter Testing, **Then** the testing_started_at timestamp is recorded for tracking.

---

### User Story 3 - Configure AI Review Settings (Priority: P2)

As a project administrator, I want to configure AI Review behavior so that I can control iteration limits and customize review prompts for my project's quality standards.

**Why this priority**: AI Review settings affect code quality and team efficiency. Having configurable limits prevents infinite loops and ensures human oversight when needed.

**Independent Test**: Admin sets max AI review iterations and optionally provides a custom review prompt. Tasks undergoing AI Review respect these limits and use custom prompts when configured.

**Acceptance Scenarios**:

1. **Given** max AI Review iterations is set to N, **When** AI Review exceeds N iterations without passing, **Then** the task stays in InReview for manual intervention.

2. **Given** a custom AI Review prompt template is configured, **When** AI Review starts, **Then** the custom prompt is used instead of the default.

3. **Given** AI Review iterations tracking, **When** viewing task details, **Then** the current iteration count is visible.

---

### User Story 4 - View Workflow Configuration (Priority: P2)

As a team member, I want to view the current project's workflow configuration so that I understand the expected workflow and status transition rules.

**Why this priority**: Transparency into workflow rules helps team members understand expected behavior and reduces confusion about task status transitions.

**Independent Test**: Any project member accesses project settings and views workflow configuration. The current settings are displayed without edit capability.

**Acceptance Scenarios**:

1. **Given** a user is a member of a project, **When** they access project settings, **Then** they can view the current workflow configuration (read-only for non-admins).

2. **Given** workflow configuration includes Human Review setting, **When** viewing settings, **Then** the current state (enabled/disabled) is clearly visible.

---

### Edge Cases

- Configuration changes apply immediately to all tasks regardless of current status
- How does the system handle invalid configuration combinations?
- What occurs when the project admin is removed?
- How does configuration migration work when upgrading from projects without workflow settings?
- max_ai_review_iterations must be >= 1 (0 is rejected as invalid)

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST store workflow configuration per-project, not per-user
- **FR-002**: System MUST provide API endpoints to read and update project workflow settings
- **FR-003**: System MUST validate status transitions against project-level workflow configuration
- **FR-004**: System MUST allow Human Review phase to be enabled or disabled per-project
- **FR-005**: System MUST allow Testing phase to require manual exit or allow bypass per-project
- **FR-006**: System MUST enforce maximum AI Review iterations per-project
- **FR-007**: System MUST record testing_started_at timestamp when tasks enter Testing phase
- **FR-008**: System MUST track AI Review iteration count on tasks
- **FR-009**: Users with project admin role MUST be able to modify workflow settings
- **FR-010**: All project members MUST be able to view workflow settings (read-only)

### Key Entities

- **ProjectWorkflowConfig**: Configuration settings for a project's workflow behavior
  - `project_id`: Reference to the project
  - `enable_human_review`: Boolean - whether Human Review phase is enabled
  - `testing_requires_manual_exit`: Boolean - whether Testing requires explicit exit
  - `max_ai_review_iterations`: Integer - maximum AI Review attempts before manual intervention
  - `ai_review_prompt_template`: Optional custom prompt for AI Review

- **TaskWorkflowTracking**: Tracking fields on tasks for workflow state
  - `testing_started_at`: Timestamp when task entered Testing phase
  - `ai_review_iterations`: Count of AI Review attempts

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Project administrators can configure workflow settings and changes persist across sessions
- **SC-002**: Status transitions respect project workflow configuration (Human Review skipped when disabled)
- **SC-003**: All project members can view current workflow configuration
- **SC-004**: Workflow configuration changes take effect within 5 seconds of saving
- **SC-005**: Existing projects without workflow settings receive default configuration on first access

## Assumptions

- Project admin role and permissions system already exists
- Project table can be extended with JSON or individual columns for workflow settings
- Existing status transition validation can accept project-level configuration
- User interface will reuse existing settings panel patterns
- Default workflow configuration matches current hardcoded behavior (Human Review disabled by default)
