# API Contract: Project Workflow Configuration

**Feature**: Project-Level Workflow Settings
**Date**: 2026-01-09

## GET /projects/:id/workflow-config

Get the current workflow configuration for a project.

### Request

```
GET /projects/:id/workflow-config
Authorization: Bearer <token>
```

### Response (200 OK)

```json
{
  "data": {
    "enable_human_review": false,
    "max_ai_review_iterations": 3,
    "testing_requires_manual_exit": true,
    "auto_start_ai_review": true,
    "ai_review_prompt_template": null
  }
}
```

### Response (404 Not Found)

```json
{
  "error": {
    "code": "PROJECT_NOT_FOUND",
    "message": "Project not found"
  }
}
```

### Response (403 Forbidden)

```json
{
  "error": {
    "code": "FORBIDDEN",
    "message": "You do not have permission to view this project's workflow settings"
  }
}
```

---

## PATCH /projects/:id/workflow-config

Update the workflow configuration for a project. Only project admins can modify settings.

### Request

```
PATCH /projects/:id/workflow-config
Authorization: Bearer <token>
Content-Type: application/json

{
  "enable_human_review": true,
  "max_ai_review_iterations": 5,
  "testing_requires_manual_exit": false,
  "auto_start_ai_review": true,
  "ai_review_prompt_template": "Custom review prompt for {repo}"
}
```

### Request Body Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| enable_human_review | boolean | No | Enable Human Review phase |
| max_ai_review_iterations | integer | No | Max AI Review iterations (>= 1) |
| testing_requires_manual_exit | boolean | No | Require manual exit from Testing |
| auto_start_ai_review | boolean | No | Auto-start AI Review after Testing |
| ai_review_prompt_template | string/null | No | Custom AI Review prompt |

### Response (200 OK)

```json
{
  "data": {
    "enable_human_review": true,
    "max_ai_review_iterations": 5,
    "testing_requires_manual_exit": false,
    "auto_start_ai_review": true,
    "ai_review_prompt_template": "Custom review prompt for {repo}"
  }
}
```

### Response (400 Bad Request)

Invalid configuration:

```json
{
  "error": {
    "code": "INVALID_CONFIG",
    "message": "max_ai_review_iterations must be >= 1"
  }
}
```

### Response (403 Forbidden)

```json
{
  "error": {
    "code": "FORBIDDEN",
    "message": "Only project admins can modify workflow settings"
  }
}
```

---

## WebSocket Events

### project.workflow.config.changed

Emitted when workflow configuration changes.

```json
{
  "event": "project.workflow.config.changed",
  "data": {
    "projectId": "uuid",
    "changes": {
      "enable_human_review": {
        "oldValue": false,
        "newValue": true
      }
    },
    "changedBy": "user-uuid",
    "timestamp": "2026-01-09T10:00:00Z"
  }
}
```

### task.status.transition.rejected

Emitted when a status transition is blocked by configuration.

```json
{
  "event": "task.status.transition.rejected",
  "data": {
    "taskId": "uuid",
    "fromStatus": "inprogress",
    "toStatus": "inreview",
    "reason": "testing_requires_manual_exit is enabled",
    "timestamp": "2026-01-09T10:00:00Z"
  }
}
```

---

## Configuration Rules

### Status Transition Validation

| From | To | Required Config |
|------|-----|-----------------|
| InProgress | InReview | testing_requires_manual_exit = false |
| InReview | HumanReview | enable_human_review = true |

### Default Configuration

When a project has no workflow_config, the following defaults apply:

```json
{
  "enable_human_review": false,
  "max_ai_review_iterations": 3,
  "testing_requires_manual_exit": true,
  "auto_start_ai_review": true,
  "ai_review_prompt_template": null
}
```
