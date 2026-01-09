-- Add new workflow status values to the tasks table
-- This migration adds 'testing' and 'humanreview' statuses and tracking columns
-- Status values use Rust serialization format: todo, inprogress, testing, inreview, humanreview, done, cancelled

-- SQLite doesn't support DROP CONSTRAINT, so we recreate the table
PRAGMA foreign_keys = OFF;

-- Create new table with updated CHECK constraint and new columns
-- NOTE: Must match existing schema types (BLOB for ids, TEXT for timestamps)
CREATE TABLE tasks_new (
    id BLOB NOT NULL PRIMARY KEY,
    project_id BLOB NOT NULL,
    title TEXT NOT NULL,
    description TEXT,
    status TEXT NOT NULL DEFAULT 'todo' CHECK (status IN ('todo', 'inprogress', 'testing', 'inreview', 'humanreview', 'done', 'cancelled')),
    parent_workspace_id BLOB,
    shared_task_id BLOB,
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    testing_started_at TEXT DEFAULT NULL,
    ai_review_iterations INTEGER NOT NULL DEFAULT 0,
    ai_review_feedback TEXT DEFAULT NULL
);

-- Copy data from old table - explicitly list all columns
INSERT INTO tasks_new (id, project_id, title, description, status, parent_workspace_id, shared_task_id, created_at, updated_at, testing_started_at, ai_review_iterations, ai_review_feedback)
SELECT
    id,
    project_id,
    title,
    description,
    status,
    parent_workspace_id,
    shared_task_id,
    created_at,
    updated_at,
    NULL,
    0,
    NULL
FROM tasks;

-- Drop the old table
DROP TABLE tasks;

-- Rename new table to original name
ALTER TABLE tasks_new RENAME TO tasks;

PRAGMA foreign_keys = ON;

-- Create indexes for status queries
CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);

CREATE INDEX IF NOT EXISTS idx_tasks_project_status ON tasks(project_id, status);

CREATE INDEX IF NOT EXISTS idx_tasks_testing_started ON tasks(testing_started_at DESC)
WHERE testing_started_at IS NOT NULL;
