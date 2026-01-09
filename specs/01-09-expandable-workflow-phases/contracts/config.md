# API Contract: Workflow Configuration

**Feature**: Expandable Workflow Phases
**Date**: 2026-01-09

## Endpoints

### GET /api/v1/projects/:projectId/workflow-config

Retrieve the current workflow configuration for a project.

**Response**:
```json
{
  "data": {
    "projectId": "uuid",
    "enableHumanReview": false,
    "maxAiReviewIterations": 3,
    "testingRequiresManualExit": true,
    "autoStartAiReview": true,
    "aiReviewPromptTemplate": null
  }
}
```

### PATCH /api/v1/projects/:projectId/workflow-config

Update workflow configuration. Changes apply immediately to all tasks.

**Request**:
```json
{
  "enableHumanReview": true,
  "maxAiReviewIterations": 5,
  "testingRequiresManualExit": false,
  "autoStartAiReview": true,
  "aiReviewPromptTemplate": "Custom review prompt..."
}
```

**Validation Rules**:
- `maxAiReviewIterations`: Must be between 1 and 10
- `aiReviewPromptTemplate`: Optional, max 2000 characters

**Response**:
```json
{
  "data": {
    "projectId": "uuid",
    "enableHumanReview": true,
    "maxAiReviewIterations": 5,
    "testingRequiresManualExit": false,
    "autoStartAiReview": true,
    "aiReviewPromptTemplate": "Custom review prompt..."
  }
}
```

**Errors**:
| Status | Code | Description |
|--------|------|-------------|
| 400 | VALIDATION_ERROR | Invalid field values |
| 401 | UNAUTHORIZED | User not authenticated |
| 403 | FORBIDDEN | User not project admin |
| 404 | NOT_FOUND | Project not found |

### PUT /api/v1/projects/:projectId/workflow-config/reset

Reset workflow configuration to defaults.

**Response**:
```json
{
  "data": {
    "projectId": "uuid",
    "enableHumanReview": false,
    "maxAiReviewIterations": 3,
    "testingRequiresManualExit": true,
    "autoStartAiReview": true,
    "aiReviewPromptTemplate": null
  }
}
```

## Configuration Defaults

| Field | Default | Description |
|-------|---------|-------------|
| enableHumanReview | false | Human Review phase disabled by default |
| maxAiReviewIterations | 3 | Max 3 AI review attempts |
| testingRequiresManualExit | true | Manual exit from Testing |
| autoStartAiReview | true | Auto-trigger AI review |
| aiReviewPromptTemplate | null | Use system default prompt |
