-- Add workflow configuration column to projects table
-- Stores JSON serialized ProjectWorkflowConfig for per-project workflow settings

ALTER TABLE projects ADD COLUMN workflow_config TEXT DEFAULT NULL;
