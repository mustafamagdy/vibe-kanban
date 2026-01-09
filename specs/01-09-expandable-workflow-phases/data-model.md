# Data Model: Expandable Workflow Phases

**Feature**: Expandable Workflow Phases
**Date**: 2026-01-09
**Source**: Derived from spec.md requirements

## Entities

### TaskStatus (Enum)

**Description**: Represents the current phase of a task in the workflow

**Values**:
| Status | Phase | Transitions To |
|--------|-------|----------------|
| `Todo` | Backlog | InProgress |
| `InProgress` | Active work | Testing |
| `Testing` | Verification | InReview |
| `InReview` | AI review | HumanReview (if enabled), Done |
| `HumanReview` | Human approval | Done |
| `Done` | Completed | - |
| `Cancelled` | Abandoned | - |

**Rust Definition**:
```rust
#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS,
)]
#[ts(export)]
pub enum TaskStatus {
    Todo,
    InProgress,
    Testing,
    InReview,
    HumanReview,
    Done,
    Cancelled,
}
```

**Validation Rules**:
- All valid transitions defined in status_transitions table
- Invalid transitions return error
- Status changes audited with timestamp

### WorkflowConfig (Struct)

**Description**: Project-level configuration for workflow behavior

**Fields**:
| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `enable_human_review` | bool | No | false | Require human approval before Done |
| `max_ai_review_iterations` | u32 | No | 3 | Max AI review attempts before manual intervention |
| `testing_requires_manual_exit` | bool | No | true | User must manually move from Testing |
| `auto_start_ai_review` | bool | No | true | Auto-trigger AI review after Testing |
| `ai_review_prompt_template` | Option<String> | No | None | Custom prompt for AI review |

**Rust Definition**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct WorkflowConfig {
    #[serde(default = "default_enable_human_review")]
    pub enable_human_review: bool,

    #[serde(default = "default_max_ai_review_iterations")]
    pub max_ai_review_iterations: u32,

    #[serde(default = "default_testing_requires_manual_exit")]
    pub testing_requires_manual_exit: bool,

    #[serde(default = "default_auto_start_ai_review")]
    pub auto_start_ai_review: bool,

    #[serde(default)]
    pub ai_review_prompt_template: Option<String>,
}

fn default_enable_human_review() -> bool { false }
fn default_max_ai_review_iterations() -> u32 { 3 }
fn default_testing_requires_manual_exit() -> bool { true }
fn default_auto_start_ai_review() -> bool { true }
```

### Task Entity (Extended)

**Description**: Existing task entity with workflow-related extensions

**New/Modified Fields**:
| Field | Type | Description |
|-------|------|-------------|
| `status` | TaskStatus | Current workflow phase |
| `testing_started_at` | Option<DateTime<Utc>> | When Testing phase began |
| `ai_review_iterations` | u32 | Number of AI review attempts |
| `ai_review_feedback` | Option<Json> | Last AI review feedback |

**Relationships**:
- Task → Task (parent/subtask for AI review feedback)
- Task → Project (via project_id, which has WorkflowConfig)

## Database Schema

### Migration: Add Workflow Statuses

**File**: `crates/db/migrations/YYYYMMDDHHMMSS_add_workflow_statuses.sql`

```sql
-- Add new status values to CHECK constraint
ALTER TABLE tasks
DROP CONSTRAINT IF EXISTS tasks_status_check;

ALTER TABLE tasks
ADD CONSTRAINT tasks_status_check CHECK (
    status IN ('todo', 'in_progress', 'testing', 'in_review', 'human_review', 'done', 'cancelled')
);

-- Add new columns for tracking
ALTER TABLE tasks
ADD COLUMN IF NOT EXISTS testing_started_at TIMESTAMP WITH TIME ZONE;

ALTER TABLE tasks
ADD COLUMN IF NOT EXISTS ai_review_iterations INTEGER NOT NULL DEFAULT 0;

ALTER TABLE tasks
ADD COLUMN IF NOT EXISTS ai_review_feedback JSONB;

-- Create index for status queries
CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
CREATE INDEX IF NOT EXISTS idx_tasks_project_status ON tasks(project_id, status);
```

## Status Transitions

| From | To | Condition |
|------|-----|-----------|
| Todo | InProgress | User starts task |
| InProgress | Testing | User marks ready for review |
| Testing | InReview | Testing complete (manual or auto) |
| InReview | Done | AI passes, HumanReview disabled |
| InReview | HumanReview | AI passes, HumanReview enabled |
| InReview | InProgress | AI fails (creates subtasks) |
| InReview | InReview | AI needs intervention (max iterations) |
| HumanReview | Done | Human approves |
| HumanReview | InProgress | Human rejects (returns to work) |
| Any | Cancelled | User cancels task |

## TypeScript Types (Generated)

```typescript
export type TaskStatus =
  | 'todo'
  | 'inProgress'
  | 'testing'
  | 'inReview'
  | 'humanReview'
  | 'done'
  | 'cancelled';

export interface WorkflowConfig {
  enableHumanReview: boolean;
  maxAiReviewIterations: number;
  testingRequiresManualExit: boolean;
  autoStartAiReview: boolean;
  aiReviewPromptTemplate: string | null;
}
```

## Constraints

1. **Unique**: Task status per task
2. **Required**: status field on all tasks
3. **Referential**: project_id must reference valid project
4. **Validation**: WorkflowConfig fields validated on save

## Indexes

- `idx_tasks_status` - Filter by status
- `idx_tasks_project_status` - Project + status composite
- `idx_tasks_testing_started` - Find stale testing phases
