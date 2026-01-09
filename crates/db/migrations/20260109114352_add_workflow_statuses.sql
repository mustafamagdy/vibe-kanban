-- Add new workflow status values to the tasks table
-- This migration adds 'testing' and 'human_review' statuses and tracking columns

-- Drop existing CHECK constraint and recreate with new status values
ALTER TABLE tasks
DROP CONSTRAINT IF EXISTS tasks_status_check;

ALTER TABLE tasks
ADD CONSTRAINT tasks_status_check CHECK (
    status IN ('todo', 'in_progress', 'testing', 'in_review', 'human_review', 'done', 'cancelled')
);

-- Add new columns for workflow tracking
ALTER TABLE tasks
ADD COLUMN IF NOT EXISTS testing_started_at TIMESTAMP WITH TIME ZONE;

ALTER TABLE tasks
ADD COLUMN IF NOT EXISTS ai_review_iterations INTEGER NOT NULL DEFAULT 0;

ALTER TABLE tasks
ADD COLUMN IF NOT EXISTS ai_review_feedback JSONB;

-- Create indexes for status queries
CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);

CREATE INDEX IF NOT EXISTS idx_tasks_project_status ON tasks(project_id, status);

CREATE INDEX IF NOT EXISTS idx_tasks_testing_started ON tasks(testing_started_at DESC)
WHERE testing_started_at IS NOT NULL;
