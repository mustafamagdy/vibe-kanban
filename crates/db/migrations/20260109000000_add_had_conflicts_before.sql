-- Add had_conflicts_before column to track conflict state at execution start
-- This enables auto-moving tasks to Done when conflicts are resolved by agent execution
ALTER TABLE execution_process_repo_states ADD COLUMN had_conflicts_before INTEGER NOT NULL DEFAULT 0;
