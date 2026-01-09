# Research Findings: Project-Level Workflow Settings

**Feature**: Project-Level Workflow Settings
**Date**: 2026-01-09
**Status**: Complete

## Research Questions

### R1: SQLite JSON Column Support

**Question**: How to store flexible configuration in SQLite?

**Finding**: SQLite 3.38.0+ has JSON1 extension for native JSON support. For older versions or when using SQLx, storing as TEXT with serde_json serialization is reliable.

**Decision**: Store workflow_config as TEXT column using serde_json serialization/deserialization. This pattern:
- Works across all SQLite versions
- Provides type safety via serde
- Allows future schema evolution without migration
- Enables easy debugging by viewing raw JSON

### R2: Existing Project Model Extension Pattern

**Question**: How to extend existing Project model without breaking compatibility?

**Finding**: The existing Project model in `crates/db/src/models/project.rs` uses sqlx query macros with explicit column lists. Adding an optional JSON column requires:
1. Add column to migration (NULL default)
2. Update SQLx queries to include new column
3. Add serde attributes for JSON serialization

**Decision**: Add `workflow_config` as `Option<String>` in Rust model, deserialize with serde_json when needed. Use `#[serde(default)]` to handle null values gracefully.

### R3: Status Transition Configuration Pattern

**Question**: How to pass project configuration to status transition validation?

**Finding**: The existing `validate_status_transition()` method in container.rs accepts `Option<&WorkflowConfig>` but never uses it. The pattern should be:
1. Fetch project with workflow_config
2. Pass config to validate_status_transition
3. Use config to conditionally allow/disallow transitions

**Decision**: Update validate_status_transition to:
- Check `enable_human_review` before allowing InReview → HumanReview transition
- Check `testing_requires_manual_exit` before allowing InProgress → InReview bypass
- Use defaults when config is null (maintain backward compatibility)

### R4: Frontend Settings Panel Pattern

**Question**: What frontend patterns exist for project settings?

**Finding**: The project has settings panels in `frontend/src/pages/settings/`. Settings use:
- React state for form values
- API hooks for data fetching
- Optimistic updates for better UX

**Decision**: Create new WorkflowSettings component that:
- Uses existing settings panel layout
- Fetches config via new API endpoint
- Provides toggles/inputs for each config field
- Shows validation errors for invalid values

## Decisions Summary

| Decision | Rationale |
|----------|-----------|
| TEXT column with serde_json | Maximum compatibility, type safety, debuggability |
| Optional config with defaults | Backward compatible with existing projects |
| Config-aware transitions | Enables flexible workflow customization |
| Reuse WorkflowConfig struct | No duplication, consistent with user-level config |

## Alternatives Considered

| Alternative | Why Rejected |
|-------------|--------------|
| SQLite JSON1 extension | Not available in all environments; TEXT is universally supported |
| Separate workflow_config table | Over-engineered for simple key-value config |
| Environment variables | Not project-specific; hard to change per-project |
| Hardcoded settings | No customization; defeats feature purpose |

## References

- SQLite JSON1: https://www.sqlite.org/json1.html
- serde_json: https://docs.rs/serde_json
- Existing WorkflowConfig: `crates/services/src/services/config/versions/v9.rs`
