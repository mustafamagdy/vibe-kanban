# Quickstart: Expandable Workflow Phases

**Feature**: Expandable Workflow Phases
**Date**: 2026-01-09

## Overview

This feature adds two new workflow phases (Testing, HumanReview) and configurable AI self-review to the task management system.

## Architecture

### Backend (Rust)

```
crates/db/src/models/task.rs
    └── TaskStatus enum (updated)

crates/db/migrations/*.sql
    └── New migration for status values

crates/services/src/services/config/versions/
    └── v9.rs (new WorkflowConfig)

crates/services/src/services/container.rs
    └── Status transition methods
```

### Frontend (TypeScript/React)

```
frontend/src/pages/ProjectTasks.tsx
    └── Kanban columns (Testing, HumanReview)

frontend/src/utils/statusLabels.ts
    └── New status labels

frontend/src/pages/settings/GeneralSettings.tsx
    └── Workflow configuration UI

frontend/src/i18n/locales/en/settings.json
    └── Translation strings
```

### Shared

```
shared/types.ts
    └── Auto-generated via pnpm run generate-types
```

## Setup

### 1. Database Migration

```bash
# Create migration (run from crates/db)
sqlx migrate add add_workflow_statuses

# Apply migrations
cargo run --bin migrate
```

### 2. Generate Types

```bash
pnpm run generate-types
```

### 3. Run Tests

```bash
# Backend tests
cargo test --workspace

# Frontend type check
pnpm run check
```

## Configuration

### Default Workflow (Human Review Disabled)

```
Todo → InProgress → Testing → InReview → Done
```

### Full Workflow (Human Review Enabled)

```
Todo → InProgress → Testing → InReview → HumanReview → Done
```

### Configuration Options

| Setting | Default | Description |
|---------|---------|-------------|
| enable_human_review | false | Require human approval |
| max_ai_review_iterations | 3 | Max AI review attempts |
| testing_requires_manual_exit | true | Manual Testing exit |
| auto_start_ai_review | true | Auto-trigger AI review |

## API Endpoints

### Workflow Configuration

- `GET /api/v1/projects/:id/workflow-config`
- `PATCH /api/v1/projects/:id/workflow-config`

### Task Status

- `PATCH /api/v1/tasks/:id/status`
- `POST /api/v1/tasks/:id/testing/complete`
- `POST /api/v1/tasks/:id/human-review/approve`
- `POST /api/v1/tasks/:id/human-review/reject`

## Frontend Changes

### Kanban Columns

New columns added based on config:
1. **Testing** - Appears after "In Progress"
2. **AI Review** - Renamed from "In Review"
3. **Human Review** - Conditionally shown when enabled

### Settings UI

New "Workflow" section in General Settings:
- Toggle for Human Review phase
- Toggle for Testing manual exit
- Input for max AI iterations

## Testing Checklist

- [ ] Tasks can enter Testing phase from InProgress
- [ ] Tasks can exit Testing phase (manual or auto)
- [ ] AI review triggers automatically after Testing
- [ ] AI review creates subtasks on failure
- [ ] AI review iteration limit enforced
- [ ] Human Review column appears when enabled
- [ ] Human Review approval moves task to Done
- [ ] Configuration changes apply immediately
- [ ] Existing tasks migrate correctly
- [ ] Backward compatibility with old status values

## Rollback Plan

1. **Database**: Migration is reversible via `sqlx migrate revert`
2. **Config**: v9 migrates gracefully to v8 (fields dropped with defaults)
3. **Frontend**: Old status values handled gracefully with fallbacks

## Monitoring

### Metrics to Track

- Task status transition counts
- AI review pass/fail rates
- Human Review approval times
- Testing phase duration

### Logs

- `task.status.transition` - Status change events
- `ai.review.completed` - AI review results
- `workflow.config.changed` - Config updates
