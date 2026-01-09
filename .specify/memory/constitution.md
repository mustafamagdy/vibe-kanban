<!--
  Sync Impact Report:
  - Version change: N/A → 1.0.0 (new constitution)
  - Added principles: Strict Structure Adherence, DRY Principle, Code Maintainability,
    Backend Coding Standards, Frontend Coding Standards, Shared Types Management
  - Added sections: Shared Types Management, Development Workflow
  - Templates requiring updates: None (template files don't need changes for this constitution)
  - Follow-up TODOs: None
-->

# Vibe Kanban Constitution

## Core Principles

### I. Strict Structure Adherence
All code MUST follow the defined project structure:
- **Backend (Rust)**: All crates MUST reside in `crates/` with clear separation:
  - `server`: API and binary executables
  - `db`: SQLx models and migrations
  - `executors`, `services`, `utils`: Functional modules
  - `deployment`, `local-deployment`, `remote`: Deployment concerns
- **Frontend (TypeScript/React)**: All source MUST be in `frontend/src/`
  - Components in `frontend/src/components/` with subdirectories for organization
  - Dialog components in `frontend/src/components/dialogs`
- **Shared Types**: Generated types in `shared/types.ts` - NEVER edit manually
- **CLI Tools**: Published packages in `npx-cli/`

Rationale: A consistent structure enables rapid navigation, onboarding, and maintenance across the monorepo.

### II. DRY Principle (Don't Repeat Yourself)
Code MUST be written once and reused:
- Extract common utilities to `crates/utils/` or `frontend/src/utils/`
- Share types between Rust and TypeScript via `ts-rs` generated declarations
- Reusable components MUST be extracted to shared locations
- Configuration shared across environments MUST be centralized in `scripts/` or `deployment/`

Rationale: Duplication leads to drift, bugs, and increased maintenance burden.

### III. Code Maintainability
All code MUST be maintainable:
- Functions MUST be small and focused on a single responsibility
- Rust types MUST derive `Debug`, `Serialize`, `Deserialize` where applicable
- Complex logic MUST be documented with comments explaining "why", not "what"
- Technical debt MUST be tracked and addressed proactively

Rationale: Code is read more often than written; optimize for comprehension.

### IV. Backend Coding Standards (Rust)
All Rust code MUST conform to:
- `rustfmt` formatting (configured in `rustfmt.toml`)
- Imports grouped by crate
- Snake_case for modules, PascalCase for types
- Unit tests alongside code using `#[cfg(test)]`
- Run `cargo test --workspace` before commits

Rationale: Consistent Rust style ensures readability across the backend codebase.

### V. Frontend Coding Standards (TypeScript/React)
All TypeScript/React code MUST conform to:
- ESLint + Prettier (2 spaces, single quotes, 80 column max)
- PascalCase for components, camelCase for variables/functions
- Kebab-case for file names where practical
- Type checks via `pnpm run check` before commits
- Lightweight tests (e.g., Vitest) in same directory as implementation

Rationale: Frontend consistency enables parallel development and reduces cognitive load.

### VI. Shared Types Management
TypeScript types MUST be generated from Rust:
- Use `#[derive(TS)]` and related macros on Rust structs/enums
- Regenerate types via `pnpm run generate-types` after Rust changes
- NEVER manually edit `shared/types.ts`
- The source of truth is always the Rust type definition

Rationale: Single source of truth for types prevents divergence between backend and frontend.

## Development Workflow

### Code Review Requirements
- All PRs MUST verify compliance with constitution principles
- Frontend changes MUST pass `pnpm run check` and `pnpm run lint`
- Backend changes MUST pass `cargo check` and `cargo test --workspace`
- Structure violations MUST be justified in PR description

### Build & Test Gates
- **Install**: `pnpm i`
- **Dev**: `pnpm run dev` (frontend + backend with auto-assigned ports)
- **Type Check**: `pnpm run check` (frontend), `pnpm run backend:check` (Rust)
- **Tests**: `cargo test --workspace` (Rust), project frontend tests
- **Generate Types**: `pnpm run generate-types` after Rust type changes

### Security Practices
- Use `.env` for local overrides; NEVER commit secrets
- Environment variables: `FRONTEND_PORT`, `BACKEND_PORT`, `HOST`
- Dev ports and assets managed by `scripts/setup-dev-environment.js`

## Governance

This constitution supersedes all other development practices. Amendments require:

1. Documentation of the proposed change
2. Review and approval via PR
3. Migration plan for existing code if needed
4. Version bump according to semantic versioning:
   - **MAJOR**: Backward-incompatible principle changes
   - **MINOR**: New principles or materially expanded guidance
   - **PATCH**: Clarifications, wording fixes, non-semantic refinements

Compliance verification:
- All PRs/reviews MUST verify adherence to constitution principles
- Complexity deviations MUST be documented and justified in plan.md
- Refer to `CLAUDE.md` for runtime development guidance

**Version**: 1.0.0 | **Ratified**: 2026-01-09 | **Last Amended**: 2026-01-09
