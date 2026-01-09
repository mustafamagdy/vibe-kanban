# API Contract: Task Status Transitions

**Feature**: Expandable Workflow Phases
**Date**: 2026-01-09

## Endpoints

### PATCH /api/v1/tasks/:taskId/status

Update task status. Validates transitions against workflow rules.

**Request**:
```json
{
  "status": "testing"
}
```

**Valid Status Values**:
- `todo`
- `inProgress`
- `testing`
- `inReview`
- `humanReview`
- `done`
- `cancelled`

**Response**:
```json
{
  "data": {
    "taskId": "uuid",
    "status": "testing",
    "previousStatus": "inProgress",
    "testingStartedAt": "2026-01-09T10:00:00Z"
  }
}
```

**Errors**:
| Status | Code | Description |
|--------|------|-------------|
| 400 | INVALID_STATUS | Invalid status value |
| 400 | INVALID_TRANSITION | Status transition not allowed |
| 401 | UNAUTHORIZED | User not authenticated |
| 403 | FORBIDDEN | User lacks permission |
| 404 | NOT_FOUND | Task not found |

### POST /api/v1/tasks/:taskId/testing/complete

Complete testing phase and trigger AI review.

**Request Body** (optional):
```json
{
  "notes": "All tests passing",
  "durationMinutes": 45
}
```

**Response**:
```json
{
  "data": {
    "taskId": "uuid",
    "status": "inReview",
    "aiReviewStarted": true,
    "followUpTaskId": "uuid"  // Review tracking task
  }
}
```

**Side Effects**:
- Creates follow-up task for AI review
- Sets testing_started_at timestamp
- Triggers AI review (if auto_start_ai_review enabled)

### POST /api/v1/tasks/:taskId/ai-review/result

Submit AI review result (internal endpoint).

**Request**:
```json
{
  "result": "pass",  // "pass", "fail", "needs_intervention"
  "feedback": {
    "issues": ["Issue 1", "Issue 2"],
    "score": 0.85
  },
  "subtasks": [
    {
      "title": "Fix issue 1",
      "description": "..."
    }
  ]
}
```

**Response**:
```json
{
  "data": {
    "taskId": "uuid",
    "status": "humanReview",  // or "done" or "inProgress"
    "aiReviewIterations": 2,
    "subtasksCreated": 2
  }
}
```

**Behavior by Result**:
| Result | New Status | Actions |
|--------|-----------|---------|
| pass | humanReview (if enabled) or done | None |
| fail | inProgress | Creates subtasks from feedback |
| needs_intervention | inReview | Stays for manual resolution |

### POST /api/v1/tasks/:taskId/human-review/approve

Approve task in Human Review phase.

**Response**:
```json
{
  "data": {
    "taskId": "uuid",
    "status": "done",
    "approvedBy": "user-uuid",
    "approvedAt": "2026-01-09T10:30:00Z"
  }
}
```

### POST /api/v1/tasks/:taskId/human-review/reject

Reject task in Human Review phase, returning to InProgress.

**Request**:
```json
{
  "reason": "Missing documentation"
}
```

**Response**:
```json
{
  "data": {
    "taskId": "uuid",
    "status": "inProgress",
    "rejectedBy": "user-uuid",
    "rejectedAt": "2026-01-09T10:30:00Z",
    "reason": "Missing documentation"
  }
}
```

## Workflow State Machine

```
                    ┌─────────────────────────────────────┐
                    │                                     │
                    ▼                                     │
┌──────┐    ┌──────────────┐    ┌──────────┐    ┌──────────────┐
│ Todo │───▶│ InProgress   │───▶│ Testing  │───▶│ InReview     │
└──────┘    └──────────────┘    └──────────┘    └──────────────┘
                                                      │
                    ┌───────────────────────────────────┘
                    │
                    ▼
            ┌──────────────┐         ┌──────────────┐
            │ HumanReview  │◀───────▶│ Done         │
            └──────────────┘         └──────────────┘
                    │
                    ▼
            ┌──────────────┐
            │ Cancelled    │
            └──────────────┘

InReview ──fail──▶ InProgress  (creates subtasks)
InReview ──max───▶ InReview    (manual intervention)
```

## WebSocket Events

### task.status.changed

Emitted when task status changes.

```json
{
  "event": "task.status.changed",
  "data": {
    "taskId": "uuid",
    "projectId": "uuid",
    "previousStatus": "inProgress",
    "newStatus": "testing",
    "changedBy": "user-uuid",
    "timestamp": "2026-01-09T10:00:00Z"
  }
}
```

### task.workflow.action

Emitted for workflow-specific actions.

```json
{
  "event": "task.workflow.action",
  "data": {
    "taskId": "uuid",
    "action": "testing.completed",
    "details": {
      "aiReviewStarted": true,
      "followUpTaskId": "uuid"
    }
  }
}
```
