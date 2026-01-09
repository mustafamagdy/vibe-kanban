# Research: Expandable Workflow Phases

**Feature**: Expandable Workflow Phases
**Date**: 2026-01-09
**Source**: plan.md Phase 0 research tasks

## TaskStatus Enum

**Location**: `crates/db/src/models/task.rs`

**Current State**:
```rust
pub enum TaskStatus {
    Todo,
    InProgress,
    InReview,
    Done,
    Cancelled,
}
```

**Required Changes**:
- Add `Testing` status between InProgress and InReview
- Add `HumanReview` status between InReview and Done
- Semantic rename: InReview → AI Review (label only, enum value unchanged)

**Migration Strategy**:
- Create SQL migration to update CHECK constraint
- Existing InReview tasks remain valid (map to InReview = AI Review)

**Decision**: Add two new enum variants; keep InReview as-is for backward compatibility

## Config Version Pattern

**Location**: `crates/services/src/services/config/versions/`

**Pattern from v8**:
```rust
// v8.rs structure
pub struct ConfigV8 {
    // ... existing fields
    pub workflow: Option<WorkflowConfigV8>,
}

#[derive(Deserialize, Serialize)]
pub struct WorkflowConfigV8 {
    // v8 fields
}

// Migration from v7 to v8
impl ConfigMigration for MigrationV7ToV8 {
    fn migrate(config: ConfigV7) -> Result<ConfigV8, ConfigError> {
        // transform v7 to v8
    }
}
```

**v9 Requirements**:
```rust
pub struct WorkflowConfig {
    pub enable_human_review: bool,
    pub max_ai_review_iterations: u32,
    pub testing_requires_manual_exit: bool,
    pub auto_start_ai_review: bool,
    pub ai_review_prompt_template: Option<String>,
}

impl ConfigMigration for MigrationV8ToV9 {
    fn migrate(config: ConfigV8) -> Result<ConfigV9, ConfigError> {
        Ok(ConfigV9 {
            // existing fields preserved
            workflow: WorkflowConfig {
                enable_human_review: false,  // default
                max_ai_review_iterations: 3,  // default
                testing_requires_manual_exit: true,  // default
                auto_start_ai_review: true,   // default
                ai_review_prompt_template: None,
            },
        })
    }
}
```

**Decision**: Follow existing v8 pattern; add WorkflowConfig with sensible defaults

## Frontend Status Handling

**Location**: `frontend/src/pages/ProjectTasks.tsx`

**Current Pattern**:
```typescript
const TASK_STATUSES = ['todo', 'inProgress', 'inReview', 'done', 'cancelled'] as const;

const statusLabels: Record<TaskStatus, string> = {
  todo: 'To Do',
  inProgress: 'In Progress',
  inReview: 'In Review',
  done: 'Done',
  cancelled: 'Cancelled',
};
```

**Required Changes**:
- Add `testing` and `humanreview` to TASK_STATUSES
- Add new entries to statusLabels
- Add new kanban columns for Testing and HumanReview

**Kanban Column Structure**:
```typescript
const COLUMNS = [
  { status: 'todo', title: 'To Do' },
  { status: 'inProgress', title: 'In Progress' },
  { status: 'testing', title: 'Testing' },           // NEW
  { status: 'inReview', title: 'AI Review' },        // RENAMED
  { status: 'humanreview', title: 'Human Review' },  // NEW (conditional)
  { status: 'done', title: 'Done' },
];
```

**Decision**: Extend existing patterns; HumanReview column only renders when enabled

## Status Transition Logic

**Location**: `crates/services/src/services/container.rs`

**Current Pattern** (incomplete, inferred from spec):
```rust
pub async fn finalize_task(&mut self, task_id: &str) -> Result<(), Error> {
    // Called when task completes from InProgress
    // Currently routes to Done
}
```

**Required Changes**:
```rust
pub async fn finalize_task(&mut self, task_id: &str) -> Result<(), Error> {
    // Route to Testing instead of Done
    self.set_task_status(task_id, TaskStatus::Testing)?;
}

pub async fn complete_testing(&mut self, task_id: &str) -> Result<(), Error> {
    // Called when testing phase exits
    // Triggers AI review automatically
    self.trigger_ai_self_review(task_id).await?;
}

pub async fn trigger_ai_self_review(&mut self, task_id: &str) -> Result<(), Error> {
    // Creates follow-up task with review prompt
    // Handles pass/fail/iteration logic
}

pub async fn handle_ai_review_result(
    &mut self,
    task_id: &str,
    result: AiReviewResult,
) -> Result<(), Error> {
    match result {
        AiReviewResult::Pass => {
            // Check if HumanReview enabled
            // Route to HumanReview or Done
        }
        AiReviewResult::Fail { subtasks } => {
            // Create subtasks for each issue
            // Revert to InProgress
        }
        AiReviewResult::NeedsIntervention => {
            // Stay in InReview for manual resolution
        }
    }
}
```

**Decision**: Extend container.rs with new methods; integrate with existing AI review logic

## Best Practices Identified

1. **Database Migrations**: Always create new migration; never modify existing ones
2. **Config Versioning**: Increment version; provide default values for new fields
3. **Status Handling**: Preserve backward compatibility; existing statuses remain valid
4. **Frontend Components**: Extend existing arrays; avoid creating parallel structures
5. **Testing**: Add unit tests alongside new logic; use `#[cfg(test)]`

## Alternatives Considered

| Approach | Rationale |
|----------|-----------|
| Single enum with status groups | Rejected: Too complex for current needs |
| Separate workflow table | Rejected: Over-engineering; config suffices |
| Frontend-only status filtering | Rejected: Backend validation required |

## Conclusion

All technical decisions align with existing codebase patterns. No blockers identified. Proceed to Phase 1 design.
