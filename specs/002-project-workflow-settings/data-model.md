# Data Model: Project-Level Workflow Settings

**Feature**: Project-Level Workflow Settings
**Date**: 2026-01-09

## Entities

### Project (Extended)

**Description**: Core project entity with added workflow configuration

**Storage**: SQLite `projects` table

**Fields**:
| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| id | UUID | Yes | - | Primary key |
| name | TEXT | Yes | - | Project name |
| dev_script | TEXT | No | NULL | Development script path |
| dev_script_working_dir | TEXT | No | NULL | Working directory for dev script |
| default_agent_working_dir | TEXT | No | NULL | Default agent working directory |
| remote_project_id | UUID | No | NULL | Linked remote project |
| workflow_config | TEXT | No | NULL | JSON serialized workflow settings |
| created_at | DATETIME | Yes | CURRENT_TIMESTAMP | Creation timestamp |
| updated_at | DATETIME | Yes | CURRENT_TIMESTAMP | Last update timestamp |

### Task (Extended - from existing feature)

**Description**: Task entity with workflow tracking fields (added in 01-09)

**Storage**: SQLite `tasks` table

**Fields** (existing + new tracking):
| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| id | UUID | Yes | - | Primary key |
| project_id | UUID | Yes | - | Foreign key to Project |
| title | TEXT | Yes | - | Task title |
| description | TEXT | No | NULL | Task description |
| status | TEXT | Yes | 'todo' | Current status (enum) |
| parent_workspace_id | UUID | No | NULL | Parent workspace reference |
| shared_task_id | UUID | No | NULL | Shared task reference |
| created_at | DATETIME | Yes | CURRENT_TIMESTAMP | Creation timestamp |
| updated_at | DATETIME | Yes | CURRENT_TIMESTAMP | Last update timestamp |
| testing_started_at | DATETIME | No | NULL | When task entered Testing |
| ai_review_iterations | INTEGER | No | 0 | AI Review attempt count |
| ai_review_feedback | TEXT | No | NULL | AI Review feedback notes |

## Value Objects

### ProjectWorkflowConfig

**Description**: Workflow configuration settings for a project

**Serialization**: JSON stored as TEXT in workflow_config column

**Structure**:
```json
{
  "enable_human_review": false,
  "max_ai_review_iterations": 3,
  "testing_requires_manual_exit": true,
  "auto_start_ai_review": true,
  "ai_review_prompt_template": null
}
```

**Fields**:
| Field | Type | Required | Default | Validation |
|-------|------|----------|---------|------------|
| enable_human_review | boolean | No | false | - |
| max_ai_review_iterations | integer | No | 3 | Must be >= 1 |
| testing_requires_manual_exit | boolean | No | true | - |
| auto_start_ai_review | boolean | No | true | - |
| ai_review_prompt_template | string/null | No | null | Max 2000 chars |

## Relationships

```
Project 1──────>* Task
  |
  └-- workflow_config: 0..1 ProjectWorkflowConfig
```

- One Project has zero or one WorkflowConfig
- One Project has zero or many Tasks
- Each Task belongs to exactly one Project

## Status Transitions (Configuration-Aware)

### Transition Rules

| From | To | Condition |
|------|-----|-----------|
| Todo | InProgress | Always allowed |
| InProgress | Testing | Always allowed |
| InProgress | InReview | Only if testing_requires_manual_exit = false |
| Testing | InReview | Always allowed |
| Testing | Done | Always allowed |
| Testing | Cancelled | Always allowed |
| InReview | HumanReview | Only if enable_human_review = true |
| InReview | Done | Always allowed |
| InReview | Cancelled | Always allowed |
| HumanReview | Done | Always allowed |
| HumanReview | InProgress | Only if rejected, returns for revisions |
| HumanReview | Cancelled | Always allowed |

## Default Configuration

For projects without explicit workflow_config:

```json
{
  "enable_human_review": false,
  "max_ai_review_iterations": 3,
  "testing_requires_manual_exit": true,
  "auto_start_ai_review": true,
  "ai_review_prompt_template": null
}
```

This default maintains backward compatibility with existing workflow behavior.
