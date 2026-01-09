# Quick Start: Project-Level Workflow Settings

**Feature**: Project-Level Workflow Settings
**Date**: 2026-01-09

## Integration Scenarios

### Scenario 1: New Project Creation

**Context**: User creates a new project

**Flow**:
1. Project created with NULL workflow_config
2. On first access to project settings, defaults are applied
3. workflow_config set to default JSON values

**Code Points**:
- `Project::create()` - handles new project creation
- `ProjectWorkflowConfig::default()` - provides default values

**Testing**:
- Verify project has NULL workflow_config after creation
- Verify settings page shows defaults on first load

---

### Scenario 2: Modifying Human Review Setting

**Context**: Admin enables Human Review for their project

**Flow**:
1. Admin navigates to Project Settings → Workflow tab
2. Toggles "Enable Human Review" to ON
3. PATCH request sent to `/projects/:id/workflow-config`
4. Configuration saved, returns updated config
5. Frontend updates local state
6. New status transitions now allow InReview → HumanReview

**API**:
```
PATCH /projects/:id/workflow-config
Body: { "enable_human_review": true }
Response: { "data": { ..., "enable_human_review": true, ... } }
```

**Code Points**:
- `projects.rs:update_workflow_config()` - handles config update
- `container.rs:validate_status_transition()` - now allows HumanReview

**Testing**:
- Verify Human Review toggle saves correctly
- Verify Human Review column appears in kanban
- Verify tasks can transition to Human Review

---

### Scenario 3: Status Transition with Configuration

**Context**: Task status change with project config applied

**Flow**:
1. User drags task from InProgress to InReview
2. Backend validates transition against project config
3. If testing_requires_manual_exit = true: REJECT
4. If testing_requires_manual_exit = false or Testing already visited: ALLOW
5. Task status updated, WebSocket event emitted

**Code Points**:
- `container.rs:validate_status_transition()` - checks config
- `tasks.rs:update_task()` - calls validation before update

**Testing**:
- Verify InProgress → InReview fails when Testing required
- Verify InProgress → InReview succeeds when Testing bypass allowed

---

### Scenario 4: AI Review Iteration Limit Hit

**Context**: Task reaches max AI Review iterations without passing

**Flow**:
1. AI Review runs (auto_start_ai_review = true)
2. Iteration count increments
3. Iteration count >= max_ai_review_iterations
4. Task stays in InReview for manual intervention
5. Iteration count visible in task details

**Code Points**:
- `container.rs:complete_testing()` - increments iteration count
- `container.rs:check_ai_review_limits()` - enforces max iterations

**Testing**:
- Verify iteration count increments on each AI Review
- Verify task cannot auto-progress after hitting limit
- Verify iteration count displayed in UI

---

### Scenario 5: Existing Project Migration

**Context**: Project created before this feature

**Flow**:
1. User with existing project accesses settings
2. System detects NULL workflow_config
3. Default configuration initialized and saved
4. Project now has explicit config matching old behavior

**Code Points**:
- `Project::find_by_id()` - returns project without workflow_config
- `WorkflowConfig::default()` - applied when null
- `Project::update()` - saves initialized config

**Testing**:
- Verify existing projects get default config on first settings access
- Verify no behavior change for existing projects after migration

---

## API Reference

### Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | /projects/:id/workflow-config | Get current workflow configuration |
| PATCH | /projects/:id/workflow-config | Update workflow configuration |

### Error Responses

| Status | Code | Description |
|--------|------|-------------|
| 400 | INVALID_CONFIG | Invalid configuration value |
| 400 | INVALID_TRANSITION | Transition not allowed by config |
| 401 | UNAUTHORIZED | User not authenticated |
| 403 | FORBIDDEN | User lacks project admin role |
| 404 | NOT_FOUND | Project not found |

### Configuration Validation

- `max_ai_review_iterations`: Must be >= 1, integer
- `enable_human_review`: Boolean
- `testing_requires_manual_exit`: Boolean
- `auto_start_ai_review`: Boolean
- `ai_review_prompt_template`: Optional string, max 2000 chars
