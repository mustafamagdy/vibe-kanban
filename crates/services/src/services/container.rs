use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Error as AnyhowError, anyhow};
use async_trait::async_trait;
use db::{
    DBService,
    models::{
        coding_agent_turn::{CodingAgentTurn, CreateCodingAgentTurn},
        execution_process::{
            CreateExecutionProcess, ExecutionContext, ExecutionProcess, ExecutionProcessRunReason,
            ExecutionProcessStatus,
        },
        execution_process_logs::ExecutionProcessLogs,
        execution_process_repo_state::{
            CreateExecutionProcessRepoState, ExecutionProcessRepoState,
        },
        project::{Project, UpdateProject},
        project_repo::{ProjectRepo, ProjectRepoWithName},
        repo::Repo,
        session::{CreateSession, Session, SessionError},
        task::{CreateTask, Task, TaskStatus},
        workspace::{Workspace, WorkspaceError},
        workspace_repo::WorkspaceRepo,
    },
};
use executors::{
    actions::{
        ExecutorAction, ExecutorActionType,
        coding_agent_initial::CodingAgentInitialRequest,
        script::{ScriptContext, ScriptRequest, ScriptRequestLanguage},
    },
    executors::{ExecutorError, StandardCodingAgentExecutor},
    logs::{NormalizedEntry, NormalizedEntryError, NormalizedEntryType, utils::ConversationPatch},
    profile::{ExecutorConfigs, ExecutorProfileId},
};
use futures::{StreamExt, future};
use sqlx::Error as SqlxError;
use thiserror::Error;
use tokio::{sync::RwLock, task::JoinHandle};
use utils::{
    log_msg::LogMsg,
    msg_store::MsgStore,
    text::{git_branch_id, short_uuid},
};
use uuid::Uuid;

use crate::services::{
    git::{GitService, GitServiceError},
    notification::NotificationService,
    share::SharePublisher,
    workspace_manager::WorkspaceError as WorkspaceManagerError,
    worktree_manager::WorktreeError,
};
pub type ContainerRef = String;

#[derive(Debug, Error)]
pub enum ContainerError {
    #[error(transparent)]
    GitServiceError(#[from] GitServiceError),
    #[error(transparent)]
    Sqlx(#[from] SqlxError),
    #[error(transparent)]
    ExecutorError(#[from] ExecutorError),
    #[error(transparent)]
    Worktree(#[from] WorktreeError),
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
    #[error(transparent)]
    WorkspaceManager(#[from] WorkspaceManagerError),
    #[error(transparent)]
    Session(#[from] SessionError),
    #[error("Io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to kill process: {0}")]
    KillFailed(std::io::Error),
    #[error(transparent)]
    Other(#[from] AnyhowError), // Catches any unclassified errors
    #[error("Invalid status transition: {0}")]
    InvalidStatusTransition(&'static str),
}

#[async_trait]
pub trait ContainerService {
    fn msg_stores(&self) -> &Arc<RwLock<HashMap<Uuid, Arc<MsgStore>>>>;

    fn db(&self) -> &DBService;

    fn git(&self) -> &GitService;

    fn share_publisher(&self) -> Option<&SharePublisher>;

    fn notification_service(&self) -> &NotificationService;

    fn workspace_to_current_dir(&self, workspace: &Workspace) -> PathBuf;

    async fn create(&self, workspace: &Workspace) -> Result<ContainerRef, ContainerError>;

    async fn kill_all_running_processes(&self) -> Result<(), ContainerError>;

    async fn delete(&self, workspace: &Workspace) -> Result<(), ContainerError>;

    /// Check if a task has any running execution processes
    async fn has_running_processes(&self, task_id: Uuid) -> Result<bool, ContainerError> {
        let workspaces = Workspace::fetch_all(&self.db().pool, Some(task_id)).await?;

        for workspace in workspaces {
            let sessions = Session::find_by_workspace_id(&self.db().pool, workspace.id).await?;
            for session in sessions {
                if let Ok(processes) =
                    ExecutionProcess::find_by_session_id(&self.db().pool, session.id, false).await
                {
                    for process in processes {
                        if process.status == ExecutionProcessStatus::Running {
                            return Ok(true);
                        }
                    }
                }
            }
        }

        Ok(false)
    }

    /// A context is finalized when
    /// - Always when the execution process has failed or been killed
    /// - Never when the run reason is DevServer
    /// - Never when a setup script has no next_action (parallel mode)
    /// - The next action is None (no follow-up actions)
    fn should_finalize(&self, ctx: &ExecutionContext) -> bool {
        // Never finalize DevServer processes
        if matches!(
            ctx.execution_process.run_reason,
            ExecutionProcessRunReason::DevServer
        ) {
            return false;
        }

        // Never finalize setup scripts without a next_action (parallel mode).
        // In sequential mode, setup scripts have next_action pointing to coding agent,
        // so they won't finalize anyway (handled by next_action.is_none() check below).
        let action = ctx.execution_process.executor_action().unwrap();
        if matches!(
            ctx.execution_process.run_reason,
            ExecutionProcessRunReason::SetupScript
        ) && action.next_action.is_none()
        {
            return false;
        }

        // Always finalize failed or killed executions, regardless of next action
        if matches!(
            ctx.execution_process.status,
            ExecutionProcessStatus::Failed | ExecutionProcessStatus::Killed
        ) {
            return true;
        }

        // Otherwise, finalize only if no next action
        action.next_action.is_none()
    }

    /// Check if conflicts were resolved during execution.
    /// Returns true if any repo had conflicts before execution AND no conflicts remain after.
    async fn check_conflicts_resolved(
        &self,
        ctx: &ExecutionContext,
    ) -> Result<bool, ContainerError> {
        let repo_states =
            ExecutionProcessRepoState::find_by_execution_process_id(&self.db().pool, ctx.execution_process.id)
                .await?;

        // Check if any repo had conflicts before
        let had_conflicts_before = repo_states.iter().any(|state| state.had_conflicts_before);
        if !had_conflicts_before {
            return Ok(false);
        }

        // Get workspace root for path resolution
        let workspace_root = ctx
            .workspace
            .container_ref
            .as_ref()
            .map(std::path::PathBuf::from)
            .ok_or_else(|| ContainerError::Other(anyhow!("Container ref not found")))?;

        // Fetch all repos in a single query (avoids N+1)
        let repo_ids: Vec<_> = repo_states.iter().map(|s| s.repo_id).collect();
        let repos = Repo::find_by_ids(&self.db().pool, &repo_ids).await?;
        let repos_map: std::collections::HashMap<Uuid, Repo> = repos.into_iter().map(|r| (r.id, r)).collect();

        // Check if all repos still have conflicts
        for state in &repo_states {
            let repo = repos_map.get(&state.repo_id)
                .ok_or_else(|| ContainerError::Other(anyhow!("Repo not found: {}", state.repo_id)))?;

            let repo_path = workspace_root.join(&repo.name);

            // Check for current conflicts
            let has_rebase = self.git().is_rebase_in_progress(&repo_path)?;
            let conflicted_files = self.git().get_conflicted_files(&repo_path)?;
            let has_conflicts = has_rebase || !conflicted_files.is_empty();

            if has_conflicts {
                // Conflicts still exist, not resolved
                return Ok(false);
            }
        }

        // All conflicts have been resolved
        Ok(true)
    }

    /// Finalize task execution by updating status to Testing (new workflow phase) and sending notifications
    /// The workflow is: Todo → InProgress → Testing → AI Review → Human Review (optional) → Done
    async fn finalize_task(
        &self,
        share_publisher: Option<&SharePublisher>,
        ctx: &ExecutionContext,
    ) {
        // Skip status update and notification if process was intentionally killed by user
        if matches!(ctx.execution_process.status, ExecutionProcessStatus::Killed) {
            return;
        }

        // Determine final status: Route to Testing phase after execution completes
        // The Testing phase serves as a gate before AI Review
        let (final_status, status_message) = if matches!(ctx.execution_process.status, ExecutionProcessStatus::Completed) {
            match self.check_conflicts_resolved(ctx).await {
                Ok(true) => (TaskStatus::Testing, "ready for testing"),
                _ => (TaskStatus::Testing, "ready for testing"),
            }
        } else {
            // Failed executions also go to Testing for review
            (TaskStatus::Testing, "ready for testing")
        };

        let status_for_log = final_status.clone();
        match Task::update_status(&self.db().pool, ctx.task.id, final_status).await {
            Ok(_) => {
                tracing::info!(
                    "Updated task {} status to {:?} ({})",
                    ctx.task.id, status_for_log, status_message
                );
                if let Some(publisher) = share_publisher
                    && let Err(err) = publisher.update_shared_task_by_id(ctx.task.id).await
                {
                    tracing::warn!(
                        ?err,
                        "Failed to propagate shared task update for {}",
                        ctx.task.id
                    );
                }
            }
            Err(e) => {
                tracing::error!("Failed to update task status to {:?}: {}", status_for_log, e);
            }
        }

        let title = format!("Task Ready for Testing: {}", ctx.task.title);
        let message = match ctx.execution_process.status {
            ExecutionProcessStatus::Completed => {
                format!(
                    "✅ '{}' completed and ready for testing\nBranch: {:?}\nExecutor: {:?}",
                    ctx.task.title, ctx.workspace.branch, ctx.session.executor
                )
            }
            ExecutionProcessStatus::Failed => format!(
                "⚠️ '{}' execution completed with issues, ready for testing\nBranch: {:?}\nExecutor: {:?}",
                ctx.task.title, ctx.workspace.branch, ctx.session.executor
            ),
            _ => {
                tracing::warn!(
                    "Tried to notify workspace completion for {} but process is still running!",
                    ctx.workspace.id
                );
                return;
            }
        };
        self.notification_service().notify(&title, &message).await;
    }

    /// Cleanup executions marked as running in the db, call at startup
    async fn cleanup_orphan_executions(&self) -> Result<(), ContainerError> {
        let running_processes = ExecutionProcess::find_running(&self.db().pool).await?;
        for process in running_processes {
            tracing::info!(
                "Found orphaned execution process {} for session {}",
                process.id,
                process.session_id
            );
            // Update the execution process status first
            if let Err(e) = ExecutionProcess::update_completion(
                &self.db().pool,
                process.id,
                ExecutionProcessStatus::Failed,
                None, // No exit code for orphaned processes
            )
            .await
            {
                tracing::error!(
                    "Failed to update orphaned execution process {} status: {}",
                    process.id,
                    e
                );
                continue;
            }
            // Capture after-head commit OID per repository
            if let Ok(ctx) = ExecutionProcess::load_context(&self.db().pool, process.id).await
                && let Some(ref container_ref) = ctx.workspace.container_ref
            {
                let workspace_root = PathBuf::from(container_ref);
                for repo in &ctx.repos {
                    let repo_path = workspace_root.join(&repo.name);
                    if let Ok(head) = self.git().get_head_info(&repo_path)
                        && let Err(err) = ExecutionProcessRepoState::update_after_head_commit(
                            &self.db().pool,
                            process.id,
                            repo.id,
                            &head.oid,
                        )
                        .await
                    {
                        tracing::warn!(
                            "Failed to update after_head_commit for repo {} on process {}: {}",
                            repo.id,
                            process.id,
                            err
                        );
                    }
                }
            }
            // Process marked as failed
            tracing::info!("Marked orphaned execution process {} as failed", process.id);
            // Update task status to InReview for coding agent and setup script failures
            if matches!(
                process.run_reason,
                ExecutionProcessRunReason::CodingAgent
                    | ExecutionProcessRunReason::SetupScript
                    | ExecutionProcessRunReason::CleanupScript
            ) && let Ok(Some(session)) =
                Session::find_by_id(&self.db().pool, process.session_id).await
                && let Ok(Some(workspace)) =
                    Workspace::find_by_id(&self.db().pool, session.workspace_id).await
                && let Ok(Some(task)) = workspace.parent_task(&self.db().pool).await
            {
                match Task::update_status(&self.db().pool, task.id, TaskStatus::InReview).await {
                    Ok(_) => {
                        if let Some(publisher) = self.share_publisher()
                            && let Err(err) = publisher.update_shared_task_by_id(task.id).await
                        {
                            tracing::warn!(
                                ?err,
                                "Failed to propagate shared task update for {}",
                                task.id
                            );
                        }
                    }
                    Err(e) => {
                        tracing::error!(
                            "Failed to update task status to InReview for orphaned session: {}",
                            e
                        );
                    }
                }
            }
        }
        Ok(())
    }

    /// Backfill before_head_commit for legacy execution processes.
    /// Rules:
    /// - If a process has after_head_commit and missing before_head_commit,
    ///   then set before_head_commit to the previous process's after_head_commit.
    /// - If there is no previous process, set before_head_commit to the base branch commit.
    async fn backfill_before_head_commits(&self) -> Result<(), ContainerError> {
        let pool = &self.db().pool;
        let rows = ExecutionProcess::list_missing_before_context(pool).await?;
        for row in rows {
            // Skip if no after commit at all (shouldn't happen due to WHERE)
            // Prefer previous process after-commit if present
            let mut before = row.prev_after_head_commit.clone();

            // Fallback to base branch commit OID
            if before.is_none() {
                let repo_path = std::path::Path::new(row.repo_path.as_deref().unwrap_or_default());
                match self
                    .git()
                    .get_branch_oid(repo_path, row.target_branch.as_str())
                {
                    Ok(oid) => before = Some(oid),
                    Err(e) => {
                        tracing::warn!(
                            "Backfill: Failed to resolve base branch OID for workspace {} (branch {}): {}",
                            row.workspace_id,
                            row.target_branch,
                            e
                        );
                    }
                }
            }

            if let Some(before_oid) = before
                && let Err(e) = ExecutionProcessRepoState::update_before_head_commit(
                    pool,
                    row.id,
                    row.repo_id,
                    &before_oid,
                )
                .await
            {
                tracing::warn!(
                    "Backfill: Failed to update before_head_commit for process {}: {}",
                    row.id,
                    e
                );
            }
        }

        Ok(())
    }

    /// Backfill repo names that were migrated with a sentinel placeholder.
    /// Also backfills dev_script_working_dir and agent_working_dir for single-repo projects.
    async fn backfill_repo_names(&self) -> Result<(), ContainerError> {
        let pool = &self.db().pool;
        let repos = Repo::list_needing_name_fix(pool).await?;

        if repos.is_empty() {
            return Ok(());
        }

        tracing::info!("Backfilling {} repo names", repos.len());

        for repo in repos {
            let name = repo
                .path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&repo.id.to_string())
                .to_string();

            Repo::update_name(pool, repo.id, &name, &name).await?;

            // Also update dev_script_working_dir and agent_working_dir for single-repo projects
            let project_repos = ProjectRepo::find_by_repo_id(pool, repo.id).await?;
            for pr in project_repos {
                let all_repos = ProjectRepo::find_by_project_id(pool, pr.project_id).await?;
                if all_repos.len() == 1
                    && let Some(project) = Project::find_by_id(pool, pr.project_id).await?
                {
                    let needs_dev_script_working_dir = project
                        .dev_script
                        .as_ref()
                        .map(|s| !s.is_empty())
                        .unwrap_or(false)
                        && project
                            .dev_script_working_dir
                            .as_ref()
                            .map(|s| s.is_empty())
                            .unwrap_or(true);

                    let needs_default_agent_working_dir = project
                        .default_agent_working_dir
                        .as_ref()
                        .map(|s| s.is_empty())
                        .unwrap_or(true);

                    if needs_dev_script_working_dir || needs_default_agent_working_dir {
                        Project::update(
                            pool,
                            pr.project_id,
                            &UpdateProject {
                                name: Some(project.name.clone()),
                                dev_script: project.dev_script.clone(),
                                dev_script_working_dir: if needs_dev_script_working_dir {
                                    Some(name.clone())
                                } else {
                                    project.dev_script_working_dir.clone()
                                },
                                default_agent_working_dir: if needs_default_agent_working_dir {
                                    Some(name.clone())
                                } else {
                                    project.default_agent_working_dir.clone()
                                },
                            },
                        )
                        .await?;
                    }
                }
            }
        }

        Ok(())
    }

    fn cleanup_actions_for_repos(&self, repos: &[ProjectRepoWithName]) -> Option<ExecutorAction> {
        let repos_with_cleanup: Vec<_> = repos
            .iter()
            .filter(|r| r.cleanup_script.is_some())
            .collect();

        if repos_with_cleanup.is_empty() {
            return None;
        }

        let mut iter = repos_with_cleanup.iter();
        let first = iter.next()?;
        let mut root_action = ExecutorAction::new(
            ExecutorActionType::ScriptRequest(ScriptRequest {
                script: first.cleanup_script.clone().unwrap(),
                language: ScriptRequestLanguage::Bash,
                context: ScriptContext::CleanupScript,
                working_dir: Some(first.repo_name.clone()),
            }),
            None,
        );

        for repo in iter {
            root_action = root_action.append_action(ExecutorAction::new(
                ExecutorActionType::ScriptRequest(ScriptRequest {
                    script: repo.cleanup_script.clone().unwrap(),
                    language: ScriptRequestLanguage::Bash,
                    context: ScriptContext::CleanupScript,
                    working_dir: Some(repo.repo_name.clone()),
                }),
                None,
            ));
        }

        Some(root_action)
    }

    fn setup_actions_for_repos(&self, repos: &[ProjectRepoWithName]) -> Option<ExecutorAction> {
        let repos_with_setup: Vec<_> = repos.iter().filter(|r| r.setup_script.is_some()).collect();

        if repos_with_setup.is_empty() {
            return None;
        }

        let mut iter = repos_with_setup.iter();
        let first = iter.next()?;
        let mut root_action = ExecutorAction::new(
            ExecutorActionType::ScriptRequest(ScriptRequest {
                script: first.setup_script.clone().unwrap(),
                language: ScriptRequestLanguage::Bash,
                context: ScriptContext::SetupScript,
                working_dir: Some(first.repo_name.clone()),
            }),
            None,
        );

        for repo in iter {
            root_action = root_action.append_action(ExecutorAction::new(
                ExecutorActionType::ScriptRequest(ScriptRequest {
                    script: repo.setup_script.clone().unwrap(),
                    language: ScriptRequestLanguage::Bash,
                    context: ScriptContext::SetupScript,
                    working_dir: Some(repo.repo_name.clone()),
                }),
                None,
            ));
        }

        Some(root_action)
    }

    fn setup_action_for_repo(repo: &ProjectRepoWithName) -> Option<ExecutorAction> {
        repo.setup_script.as_ref().map(|script| {
            ExecutorAction::new(
                ExecutorActionType::ScriptRequest(ScriptRequest {
                    script: script.clone(),
                    language: ScriptRequestLanguage::Bash,
                    context: ScriptContext::SetupScript,
                    working_dir: Some(repo.repo_name.clone()),
                }),
                None,
            )
        })
    }

    fn build_sequential_setup_chain(
        repos: &[&ProjectRepoWithName],
        next_action: ExecutorAction,
    ) -> ExecutorAction {
        let mut chained = next_action;
        for repo in repos.iter().rev() {
            if let Some(script) = &repo.setup_script {
                chained = ExecutorAction::new(
                    ExecutorActionType::ScriptRequest(ScriptRequest {
                        script: script.clone(),
                        language: ScriptRequestLanguage::Bash,
                        context: ScriptContext::SetupScript,
                        working_dir: Some(repo.repo_name.clone()),
                    }),
                    Some(Box::new(chained)),
                );
            }
        }
        chained
    }

    async fn try_stop(&self, workspace: &Workspace, include_dev_server: bool) {
        // stop execution processes for this workspace's sessions
        let sessions = match Session::find_by_workspace_id(&self.db().pool, workspace.id).await {
            Ok(s) => s,
            Err(_) => return,
        };

        for session in sessions {
            if let Ok(processes) =
                ExecutionProcess::find_by_session_id(&self.db().pool, session.id, false).await
            {
                for process in processes {
                    // Skip dev server processes unless explicitly included
                    if !include_dev_server
                        && process.run_reason == ExecutionProcessRunReason::DevServer
                    {
                        continue;
                    }
                    if process.status == ExecutionProcessStatus::Running {
                        self.stop_execution(&process, ExecutionProcessStatus::Killed)
                            .await
                            .unwrap_or_else(|e| {
                                tracing::debug!(
                                    "Failed to stop execution process {} for workspace {}: {}",
                                    process.id,
                                    workspace.id,
                                    e
                                );
                            });
                    }
                }
            }
        }
    }

    async fn ensure_container_exists(
        &self,
        workspace: &Workspace,
    ) -> Result<ContainerRef, ContainerError>;

    async fn is_container_clean(&self, workspace: &Workspace) -> Result<bool, ContainerError>;

    async fn start_execution_inner(
        &self,
        workspace: &Workspace,
        execution_process: &ExecutionProcess,
        executor_action: &ExecutorAction,
    ) -> Result<(), ContainerError>;

    async fn stop_execution(
        &self,
        execution_process: &ExecutionProcess,
        status: ExecutionProcessStatus,
    ) -> Result<(), ContainerError>;

    async fn try_commit_changes(&self, ctx: &ExecutionContext) -> Result<bool, ContainerError>;

    async fn copy_project_files(
        &self,
        source_dir: &Path,
        target_dir: &Path,
        copy_files: &str,
    ) -> Result<(), ContainerError>;

    /// Stream diff updates as LogMsg for WebSocket endpoints.
    async fn stream_diff(
        &self,
        workspace: &Workspace,
        stats_only: bool,
    ) -> Result<futures::stream::BoxStream<'static, Result<LogMsg, std::io::Error>>, ContainerError>;

    /// Fetch the MsgStore for a given execution ID, panicking if missing.
    async fn get_msg_store_by_id(&self, uuid: &Uuid) -> Option<Arc<MsgStore>> {
        let map = self.msg_stores().read().await;
        map.get(uuid).cloned()
    }

    async fn git_branch_prefix(&self) -> String;

    async fn git_branch_from_workspace(&self, workspace_id: &Uuid, task_title: &str) -> String {
        let task_title_id = git_branch_id(task_title);
        let prefix = self.git_branch_prefix().await;

        if prefix.is_empty() {
            format!("{}-{}", short_uuid(workspace_id), task_title_id)
        } else {
            format!("{}/{}-{}", prefix, short_uuid(workspace_id), task_title_id)
        }
    }

    async fn stream_raw_logs(
        &self,
        id: &Uuid,
    ) -> Option<futures::stream::BoxStream<'static, Result<LogMsg, std::io::Error>>> {
        if let Some(store) = self.get_msg_store_by_id(id).await {
            // First try in-memory store
            return Some(
                store
                    .history_plus_stream()
                    .filter(|msg| {
                        future::ready(matches!(
                            msg,
                            Ok(LogMsg::Stdout(..) | LogMsg::Stderr(..) | LogMsg::Finished)
                        ))
                    })
                    .boxed(),
            );
        } else {
            // Fallback: load from DB and create direct stream
            let log_records =
                match ExecutionProcessLogs::find_by_execution_id(&self.db().pool, *id).await {
                    Ok(records) if !records.is_empty() => records,
                    Ok(_) => return None, // No logs exist
                    Err(e) => {
                        tracing::error!("Failed to fetch logs for execution {}: {}", id, e);
                        return None;
                    }
                };

            let messages = match ExecutionProcessLogs::parse_logs(&log_records) {
                Ok(msgs) => msgs,
                Err(e) => {
                    tracing::error!("Failed to parse logs for execution {}: {}", id, e);
                    return None;
                }
            };

            // Direct stream from parsed messages
            let stream = futures::stream::iter(
                messages
                    .into_iter()
                    .filter(|m| matches!(m, LogMsg::Stdout(_) | LogMsg::Stderr(_)))
                    .chain(std::iter::once(LogMsg::Finished))
                    .map(Ok::<_, std::io::Error>),
            )
            .boxed();

            Some(stream)
        }
    }

    async fn stream_normalized_logs(
        &self,
        id: &Uuid,
    ) -> Option<futures::stream::BoxStream<'static, Result<LogMsg, std::io::Error>>> {
        // First try in-memory store (existing behavior)
        if let Some(store) = self.get_msg_store_by_id(id).await {
            Some(
                store
                    .history_plus_stream() // BoxStream<Result<LogMsg, io::Error>>
                    .filter(|msg| future::ready(matches!(msg, Ok(LogMsg::JsonPatch(..)))))
                    .chain(futures::stream::once(async {
                        Ok::<_, std::io::Error>(LogMsg::Finished)
                    }))
                    .boxed(),
            )
        } else {
            // Fallback: load from DB and normalize
            let log_records =
                match ExecutionProcessLogs::find_by_execution_id(&self.db().pool, *id).await {
                    Ok(records) if !records.is_empty() => records,
                    Ok(_) => return None, // No logs exist
                    Err(e) => {
                        tracing::error!("Failed to fetch logs for execution {}: {}", id, e);
                        return None;
                    }
                };

            let raw_messages = match ExecutionProcessLogs::parse_logs(&log_records) {
                Ok(msgs) => msgs,
                Err(e) => {
                    tracing::error!("Failed to parse logs for execution {}: {}", id, e);
                    return None;
                }
            };

            // Create temporary store and populate
            // Include JsonPatch messages (already normalized) and Stdout/Stderr (need normalization)
            let temp_store = Arc::new(MsgStore::new());
            for msg in raw_messages {
                if matches!(
                    msg,
                    LogMsg::Stdout(_) | LogMsg::Stderr(_) | LogMsg::JsonPatch(_)
                ) {
                    temp_store.push(msg);
                }
            }
            temp_store.push_finished();

            let process = match ExecutionProcess::find_by_id(&self.db().pool, *id).await {
                Ok(Some(process)) => process,
                Ok(None) => {
                    tracing::error!("No execution process found for ID: {}", id);
                    return None;
                }
                Err(e) => {
                    tracing::error!("Failed to fetch execution process {}: {}", id, e);
                    return None;
                }
            };

            // Get the workspace to determine correct directory
            let (workspace, _session) =
                match process.parent_workspace_and_session(&self.db().pool).await {
                    Ok(Some((workspace, session))) => (workspace, session),
                    Ok(None) => {
                        tracing::error!(
                            "No workspace/session found for session ID: {}",
                            process.session_id
                        );
                        return None;
                    }
                    Err(e) => {
                        tracing::error!(
                            "Failed to fetch workspace for session {}: {}",
                            process.session_id,
                            e
                        );
                        return None;
                    }
                };

            if let Err(err) = self.ensure_container_exists(&workspace).await {
                tracing::warn!(
                    "Failed to recreate worktree before log normalization for workspace {}: {}",
                    workspace.id,
                    err
                );
            }

            let current_dir = self.workspace_to_current_dir(&workspace);

            let executor_action = if let Ok(executor_action) = process.executor_action() {
                executor_action
            } else {
                tracing::error!(
                    "Failed to parse executor action: {:?}",
                    process.executor_action()
                );
                return None;
            };

            // Spawn normalizer on populated store
            match executor_action.typ() {
                ExecutorActionType::CodingAgentInitialRequest(request) => {
                    let executor = ExecutorConfigs::get_cached()
                        .get_coding_agent_or_default(&request.executor_profile_id);
                    executor
                        .normalize_logs(temp_store.clone(), &request.effective_dir(&current_dir));
                }
                ExecutorActionType::CodingAgentFollowUpRequest(request) => {
                    let executor = ExecutorConfigs::get_cached()
                        .get_coding_agent_or_default(&request.executor_profile_id);
                    executor
                        .normalize_logs(temp_store.clone(), &request.effective_dir(&current_dir));
                }
                _ => {
                    tracing::debug!(
                        "Executor action doesn't support log normalization: {:?}",
                        process.executor_action()
                    );
                    return None;
                }
            }
            Some(
                temp_store
                    .history_plus_stream()
                    .filter(|msg| future::ready(matches!(msg, Ok(LogMsg::JsonPatch(..)))))
                    .chain(futures::stream::once(async {
                        Ok::<_, std::io::Error>(LogMsg::Finished)
                    }))
                    .boxed(),
            )
        }
    }

    fn spawn_stream_raw_logs_to_db(&self, execution_id: &Uuid) -> JoinHandle<()> {
        let execution_id = *execution_id;
        let msg_stores = self.msg_stores().clone();
        let db = self.db().clone();

        tokio::spawn(async move {
            // Get the message store for this execution
            let store = {
                let map = msg_stores.read().await;
                map.get(&execution_id).cloned()
            };

            if let Some(store) = store {
                let mut stream = store.history_plus_stream();

                while let Some(Ok(msg)) = stream.next().await {
                    match &msg {
                        LogMsg::Stdout(_) | LogMsg::Stderr(_) => {
                            // Serialize this individual message as a JSONL line
                            match serde_json::to_string(&msg) {
                                Ok(jsonl_line) => {
                                    let jsonl_line_with_newline = format!("{jsonl_line}\n");

                                    // Append this line to the database
                                    if let Err(e) = ExecutionProcessLogs::append_log_line(
                                        &db.pool,
                                        execution_id,
                                        &jsonl_line_with_newline,
                                    )
                                    .await
                                    {
                                        tracing::error!(
                                            "Failed to append log line for execution {}: {}",
                                            execution_id,
                                            e
                                        );
                                    }
                                }
                                Err(e) => {
                                    tracing::error!(
                                        "Failed to serialize log message for execution {}: {}",
                                        execution_id,
                                        e
                                    );
                                }
                            }
                        }
                        LogMsg::SessionId(agent_session_id) => {
                            // Append this line to the database
                            if let Err(e) = CodingAgentTurn::update_agent_session_id(
                                &db.pool,
                                execution_id,
                                agent_session_id,
                            )
                            .await
                            {
                                tracing::error!(
                                    "Failed to update agent_session_id {} for execution process {}: {}",
                                    agent_session_id,
                                    execution_id,
                                    e
                                );
                            }
                        }
                        LogMsg::Finished => {
                            break;
                        }
                        LogMsg::JsonPatch(_) => continue,
                    }
                }
            }
        })
    }

    async fn start_workspace(
        &self,
        workspace: &Workspace,
        executor_profile_id: ExecutorProfileId,
    ) -> Result<ExecutionProcess, ContainerError> {
        // Create container
        self.create(workspace).await?;

        // Get parent task
        let task = workspace
            .parent_task(&self.db().pool)
            .await?
            .ok_or(SqlxError::RowNotFound)?;

        // Get parent project
        let project = task
            .parent_project(&self.db().pool)
            .await?
            .ok_or(SqlxError::RowNotFound)?;

        let project_repos =
            ProjectRepo::find_by_project_id_with_names(&self.db().pool, project.id).await?;

        let workspace = Workspace::find_by_id(&self.db().pool, workspace.id)
            .await?
            .ok_or(SqlxError::RowNotFound)?;

        // Create a session for this workspace
        let session = Session::create(
            &self.db().pool,
            &CreateSession {
                executor: Some(executor_profile_id.executor.to_string()),
            },
            Uuid::new_v4(),
            workspace.id,
        )
        .await?;

        let prompt = task.to_prompt();

        let repos_with_setup: Vec<_> = project_repos
            .iter()
            .filter(|pr| pr.setup_script.is_some())
            .collect();

        let all_parallel = repos_with_setup.iter().all(|pr| pr.parallel_setup_script);

        let cleanup_action = self.cleanup_actions_for_repos(&project_repos);

        let working_dir = workspace
            .agent_working_dir
            .as_ref()
            .filter(|dir| !dir.is_empty())
            .cloned();

        let coding_action = ExecutorAction::new(
            ExecutorActionType::CodingAgentInitialRequest(CodingAgentInitialRequest {
                prompt,
                executor_profile_id: executor_profile_id.clone(),
                working_dir,
            }),
            cleanup_action.map(Box::new),
        );

        let execution_process = if all_parallel {
            // All parallel: start each setup independently, then start coding agent
            for repo in &repos_with_setup {
                if let Some(action) = Self::setup_action_for_repo(repo)
                    && let Err(e) = self
                        .start_execution(
                            &workspace,
                            &session,
                            &action,
                            &ExecutionProcessRunReason::SetupScript,
                        )
                        .await
                {
                    tracing::warn!(?e, "Failed to start setup script in parallel mode");
                }
            }
            self.start_execution(
                &workspace,
                &session,
                &coding_action,
                &ExecutionProcessRunReason::CodingAgent,
            )
            .await?
        } else {
            // Any sequential: chain ALL setups → coding agent via next_action
            let main_action = Self::build_sequential_setup_chain(&repos_with_setup, coding_action);
            self.start_execution(
                &workspace,
                &session,
                &main_action,
                &ExecutionProcessRunReason::SetupScript,
            )
            .await?
        };

        Ok(execution_process)
    }

    async fn start_execution(
        &self,
        workspace: &Workspace,
        session: &Session,
        executor_action: &ExecutorAction,
        run_reason: &ExecutionProcessRunReason,
    ) -> Result<ExecutionProcess, ContainerError> {
        // Update task status to InProgress when starting an execution
        let task = workspace
            .parent_task(&self.db().pool)
            .await?
            .ok_or(SqlxError::RowNotFound)?;
        if task.status != TaskStatus::InProgress
            && run_reason != &ExecutionProcessRunReason::DevServer
        {
            Task::update_status(&self.db().pool, task.id, TaskStatus::InProgress).await?;

            if let Some(publisher) = self.share_publisher()
                && let Err(err) = publisher.update_shared_task_by_id(task.id).await
            {
                tracing::warn!(
                    ?err,
                    "Failed to propagate shared task update for {}",
                    task.id
                );
            }
        }
        // Create new execution process record
        // Capture current HEAD per repository as the "before" commit for this execution
        let repositories =
            WorkspaceRepo::find_repos_for_workspace(&self.db().pool, workspace.id).await?;
        if repositories.is_empty() {
            return Err(ContainerError::Other(anyhow!(
                "Workspace has no repositories configured"
            )));
        }

        let workspace_root = workspace
            .container_ref
            .as_ref()
            .map(std::path::PathBuf::from)
            .ok_or_else(|| ContainerError::Other(anyhow!("Container ref not found")))?;

        let mut repo_states = Vec::with_capacity(repositories.len());
        for repo in &repositories {
            let repo_path = workspace_root.join(&repo.name);
            let before_head_commit = self.git().get_head_info(&repo_path).ok().map(|h| h.oid);

            // Check if there are any conflicts at the start of execution
            let has_conflicts = self
                .git()
                .is_rebase_in_progress(&repo_path)
                .unwrap_or(false)
                || !self
                    .git()
                    .get_conflicted_files(&repo_path)
                    .unwrap_or_default()
                    .is_empty();

            repo_states.push(CreateExecutionProcessRepoState {
                repo_id: repo.id,
                before_head_commit,
                after_head_commit: None,
                merge_commit: None,
                had_conflicts_before: has_conflicts,
            });
        }
        let create_execution_process = CreateExecutionProcess {
            session_id: session.id,
            executor_action: executor_action.clone(),
            run_reason: run_reason.clone(),
        };

        let execution_process = ExecutionProcess::create(
            &self.db().pool,
            &create_execution_process,
            Uuid::new_v4(),
            &repo_states,
        )
        .await?;

        if let Some(prompt) = match executor_action.typ() {
            ExecutorActionType::CodingAgentInitialRequest(coding_agent_request) => {
                Some(coding_agent_request.prompt.clone())
            }
            ExecutorActionType::CodingAgentFollowUpRequest(follow_up_request) => {
                Some(follow_up_request.prompt.clone())
            }
            _ => None,
        } {
            let create_coding_agent_turn = CreateCodingAgentTurn {
                execution_process_id: execution_process.id,
                prompt: Some(prompt),
            };

            let coding_agent_turn_id = Uuid::new_v4();

            CodingAgentTurn::create(
                &self.db().pool,
                &create_coding_agent_turn,
                coding_agent_turn_id,
            )
            .await?;
        }

        if let Err(start_error) = self
            .start_execution_inner(workspace, &execution_process, executor_action)
            .await
        {
            // Mark process as failed
            if let Err(update_error) = ExecutionProcess::update_completion(
                &self.db().pool,
                execution_process.id,
                ExecutionProcessStatus::Failed,
                None,
            )
            .await
            {
                tracing::error!(
                    "Failed to mark execution process {} as failed after start error: {}",
                    execution_process.id,
                    update_error
                );
            }
            Task::update_status(&self.db().pool, task.id, TaskStatus::InReview).await?;

            // Emit stderr error message
            let log_message = LogMsg::Stderr(format!("Failed to start execution: {start_error}"));
            if let Ok(json_line) = serde_json::to_string(&log_message) {
                let _ = ExecutionProcessLogs::append_log_line(
                    &self.db().pool,
                    execution_process.id,
                    &format!("{json_line}\n"),
                )
                .await;
            }

            // Emit NextAction with failure context for coding agent requests
            if let ContainerError::ExecutorError(ExecutorError::ExecutableNotFound { program }) =
                &start_error
            {
                let help_text = format!("The required executable `{program}` is not installed.");
                let error_message = NormalizedEntry {
                    timestamp: None,
                    entry_type: NormalizedEntryType::ErrorMessage {
                        error_type: NormalizedEntryError::SetupRequired,
                    },
                    content: help_text,
                    metadata: None,
                };
                let patch = ConversationPatch::add_normalized_entry(2, error_message);
                if let Ok(json_line) = serde_json::to_string::<LogMsg>(&LogMsg::JsonPatch(patch)) {
                    let _ = ExecutionProcessLogs::append_log_line(
                        &self.db().pool,
                        execution_process.id,
                        &format!("{json_line}\n"),
                    )
                    .await;
                }
            };
            return Err(start_error);
        }

        // Start processing normalised logs for executor requests and follow ups
        let workspace_root = self.workspace_to_current_dir(workspace);
        if let Some(msg_store) = self.get_msg_store_by_id(&execution_process.id).await
            && let Some((executor_profile_id, working_dir)) = match executor_action.typ() {
                ExecutorActionType::CodingAgentInitialRequest(request) => Some((
                    &request.executor_profile_id,
                    request.effective_dir(&workspace_root),
                )),
                ExecutorActionType::CodingAgentFollowUpRequest(request) => Some((
                    &request.executor_profile_id,
                    request.effective_dir(&workspace_root),
                )),
                _ => None,
            }
        {
            if let Some(executor) =
                ExecutorConfigs::get_cached().get_coding_agent(executor_profile_id)
            {
                executor.normalize_logs(msg_store, &working_dir);
            } else {
                tracing::error!(
                    "Failed to resolve profile '{:?}' for normalization",
                    executor_profile_id
                );
            }
        }

        self.spawn_stream_raw_logs_to_db(&execution_process.id);
        Ok(execution_process)
    }

    async fn try_start_next_action(&self, ctx: &ExecutionContext) -> Result<(), ContainerError> {
        let action = ctx.execution_process.executor_action()?;
        let next_action = if let Some(next_action) = action.next_action() {
            next_action
        } else {
            tracing::debug!("No next action configured");
            return Ok(());
        };

        // Determine the run reason of the next action
        let next_run_reason = match (action.typ(), next_action.typ()) {
            (ExecutorActionType::ScriptRequest(_), ExecutorActionType::ScriptRequest(_)) => {
                ExecutionProcessRunReason::SetupScript
            }
            (
                ExecutorActionType::CodingAgentInitialRequest(_)
                | ExecutorActionType::CodingAgentFollowUpRequest(_),
                ExecutorActionType::ScriptRequest(_),
            ) => ExecutionProcessRunReason::CleanupScript,
            (
                _,
                ExecutorActionType::CodingAgentFollowUpRequest(_)
                | ExecutorActionType::CodingAgentInitialRequest(_),
            ) => ExecutionProcessRunReason::CodingAgent,
        };

        self.start_execution(&ctx.workspace, &ctx.session, next_action, &next_run_reason)
            .await?;

        tracing::debug!("Started next action: {:?}", next_action);
        Ok(())
    }

    // =========================================================================
    // Status Transition Methods for Expandable Workflow Phases
    // =========================================================================

    /// Validate that a status transition is allowed based on project workflow config.
    /// Returns Ok(()) if valid, Err(reason) if invalid.
    fn validate_status_transition(
        &self,
        from: TaskStatus,
        to: TaskStatus,
        config: Option<&super::config::ProjectWorkflowConfig>,
    ) -> Result<(), &'static str> {
        use TaskStatus::*;

        // Self-transitions are always allowed (no-op)
        if from == to {
            return Ok(());
        }

        // Get config values with defaults (maintain backward compatibility)
        let enable_human_review = config.map(|c| c.enable_human_review).unwrap_or(false);
        let testing_requires_manual_exit = config.map(|c| c.testing_requires_manual_exit).unwrap_or(true);

        // Guard conditions before match
        let is_testing_target = matches!(to, Testing);
        let is_human_review_target = matches!(to, HumanReview);
        let from_not_in_progress = !matches!(from, InProgress);
        let from_not_in_review = !matches!(from, InReview);

        match (from, to) {
            // Valid transitions
            (Todo, InProgress) => Ok(()),
            (InProgress, Testing) => Ok(()),
            (Testing, InReview) => Ok(()),
            (Testing, InProgress) => Ok(()), // Return to InProgress for revisions
            (Testing, Done) => Ok(()),
            (Testing, Cancelled) => Ok(()),
            (InReview, Done) => Ok(()),
            (InReview, Cancelled) => Ok(()),
            (HumanReview, Done) => Ok(()),
            (HumanReview, InProgress) => Ok(()), // Rejected back to work
            (HumanReview, Cancelled) => Ok(()),
            (InProgress, Done) => Ok(()),
            (InProgress, Cancelled) => Ok(()),
            (Todo, Cancelled) => Ok(()),

            // Human Review requires config enablement
            (InReview, HumanReview) => {
                if enable_human_review {
                    Ok(())
                } else {
                    Err("Human Review is not enabled for this project")
                }
            }

            // Testing bypass requires testing_requires_manual_exit = false
            (InProgress, InReview) => {
                if testing_requires_manual_exit {
                    Err("Testing phase requires manual exit - tasks must go through Testing before AI Review")
                } else {
                    Ok(())
                }
            }

            // Invalid transitions - specific cases first
            (InReview, InProgress) => Err("Use AI review result handler instead"),
            _ if is_testing_target && from_not_in_progress => {
                Err("Only InProgress tasks can enter Testing")
            }
            _ if is_human_review_target && from_not_in_review => {
                Err("Only InReview tasks can enter Human Review")
            }

            // Invalid transitions - catch-all
            (Todo, _) => Err("Tasks must start in InProgress"),
            (_, Todo) => Err("Cannot transition to Todo status"),
            (Done, _) => Err("Cannot transition from Done"),
            (Cancelled, _) => Err("Cannot transition from Cancelled"),
            _ => Err("Invalid status transition"),
        }
    }

    /// Complete the Testing phase and transition to AI Review.
    /// Returns the updated task status.
    async fn complete_testing(
        &self,
        task_id: Uuid,
        share_publisher: Option<&SharePublisher>,
    ) -> Result<TaskStatus, ContainerError> {
        let task = Task::find_by_id(&self.db().pool, task_id)
            .await?
            .ok_or_else(|| ContainerError::Other(anyhow!("Task not found: {}", task_id)))?;

        // Load project workflow config for validation
        let config = self.get_workflow_config(task.project_id).await?;

        // Validate transition
        self.validate_status_transition(task.status, TaskStatus::InReview, Some(&config))
            .map_err(ContainerError::InvalidStatusTransition)?;

        Task::update_status(&self.db().pool, task_id, TaskStatus::InReview).await?;

        tracing::info!(
            "Task {} transitioned from Testing to AI Review",
            task_id
        );

        // Trigger AI self-review after testing
        self.trigger_ai_self_review(task_id).await?;

        if let Some(publisher) = share_publisher
            && let Err(err) = publisher.update_shared_task_by_id(task_id).await
        {
            tracing::warn!(
                ?err,
                "Failed to propagate shared task update for {}",
                task_id
            );
        }

        Ok(TaskStatus::InReview)
    }

    /// Trigger AI self-review for a task after Testing phase completion.
    /// This initiates the AI review process for quality verification.
    async fn trigger_ai_self_review(&self, task_id: Uuid) -> Result<(), ContainerError> {
        let task = Task::find_by_id(&self.db().pool, task_id)
            .await?
            .ok_or_else(|| ContainerError::Other(anyhow!("Task not found: {}", task_id)))?;

        // Get project configuration for AI review settings
        let config = self.get_workflow_config(task.project_id).await?;

        // TODO: Implement actual AI review trigger logic here
        // This would typically:
        // 1. Fetch the task execution results
        // 2. Generate AI review prompt
        // 3. Execute AI review asynchronously
        // 4. Schedule result handling

        tracing::info!(
            "Triggered AI self-review for task {} (max iterations: {})",
            task_id,
            config.max_ai_review_iterations
        );

        Ok(())
    }

    /// Handle the result of an AI review.
    /// Returns the new task status based on review outcome.
    async fn handle_ai_review_result(
        &self,
        task_id: Uuid,
        result: AIReviewResult,
        share_publisher: Option<&SharePublisher>,
    ) -> Result<TaskStatus, ContainerError> {
        let task = Task::find_by_id(&self.db().pool, task_id)
            .await?
            .ok_or_else(|| ContainerError::Other(anyhow!("Task not found: {}", task_id)))?;

        let config = self.get_workflow_config(task.project_id).await?;

        match result {
            AIReviewResult::Pass => {
                // If human review is enabled, go there; otherwise done
                let new_status = if config.enable_human_review {
                    TaskStatus::HumanReview
                } else {
                    TaskStatus::Done
                };

                let status_for_log = new_status.clone();
                Task::update_status(&self.db().pool, task_id, new_status).await?;
                tracing::info!("Task {} passed AI review, status: {:?}", task_id, status_for_log);

                if let Some(publisher) = share_publisher
                    && let Err(err) = publisher.update_shared_task_by_id(task_id).await
                {
                    tracing::warn!(
                        ?err,
                        "Failed to propagate shared task update for {}",
                        task_id
                    );
                }

                Ok(TaskStatus::Done)
            }
            AIReviewResult::Fail { issues } => {
                // Create subtasks for each issue and return to InProgress
                self.create_review_feedback_subtasks(task_id, &issues).await?;

                Task::update_status(&self.db().pool, task_id, TaskStatus::InProgress).await?;
                tracing::warn!(
                    "Task {} failed AI review with {} issues, returned to InProgress",
                    task_id,
                    issues.len()
                );

                if let Some(publisher) = share_publisher
                    && let Err(err) = publisher.update_shared_task_by_id(task_id).await
                {
                    tracing::warn!(
                        ?err,
                        "Failed to propagate shared task update for {}",
                        task_id
                    );
                }

                Ok(TaskStatus::InProgress)
            }
            AIReviewResult::NeedsIntervention => {
                // Stay in InReview for manual intervention
                tracing::info!(
                    "Task {} needs human intervention in AI Review",
                    task_id
                );
                Ok(TaskStatus::InReview)
            }
        }
    }

    /// Create subtasks from AI review feedback/issues.
    async fn create_review_feedback_subtasks(
        &self,
        parent_task_id: Uuid,
        issues: &[String],
    ) -> Result<(), ContainerError> {
        let parent_task = Task::find_by_id(&self.db().pool, parent_task_id)
            .await?
            .ok_or_else(|| ContainerError::Other(anyhow!("Task not found: {}", parent_task_id)))?;

        for issue in issues {
            let create_task = CreateTask {
                project_id: parent_task.project_id,
                title: format!("Fix: {}", issue),
                description: Some(format!("AI Review issue: {}", issue)),
                status: Some(TaskStatus::Todo),
                parent_workspace_id: None,
                image_ids: None,
                shared_task_id: None,
            };

            let _ = Task::create(&self.db().pool, &create_task, Uuid::new_v4()).await?;
            tracing::debug!("Created subtask for AI review issue: {}", issue);
        }

        Ok(())
    }

    /// Approve a task in Human Review, transitioning to Done.
    async fn approve_human_review(
        &self,
        task_id: Uuid,
        share_publisher: Option<&SharePublisher>,
    ) -> Result<TaskStatus, ContainerError> {
        let task = Task::find_by_id(&self.db().pool, task_id)
            .await?
            .ok_or_else(|| ContainerError::Other(anyhow!("Task not found: {}", task_id)))?;

        // Load project workflow config for validation
        let config = self.get_workflow_config(task.project_id).await?;

        self.validate_status_transition(task.status, TaskStatus::Done, Some(&config))
            .map_err(ContainerError::InvalidStatusTransition)?;

        Task::update_status(&self.db().pool, task_id, TaskStatus::Done).await?;
        tracing::info!("Task {} approved in Human Review, transitioned to Done", task_id);

        if let Some(publisher) = share_publisher
            && let Err(err) = publisher.update_shared_task_by_id(task_id).await
        {
            tracing::warn!(
                ?err,
                "Failed to propagate shared task update for {}",
                task_id
            );
        }

        Ok(TaskStatus::Done)
    }

    /// Reject a task in Human Review, returning to InProgress for revisions.
    async fn reject_human_review(
        &self,
        task_id: Uuid,
        reason: &str,
        share_publisher: Option<&SharePublisher>,
    ) -> Result<TaskStatus, ContainerError> {
        let task = Task::find_by_id(&self.db().pool, task_id)
            .await?
            .ok_or_else(|| ContainerError::Other(anyhow!("Task not found: {}", task_id)))?;

        // Load project workflow config for validation
        let config = self.get_workflow_config(task.project_id).await?;

        self.validate_status_transition(task.status, TaskStatus::InProgress, Some(&config))
            .map_err(ContainerError::InvalidStatusTransition)?;

        Task::update_status(&self.db().pool, task_id, TaskStatus::InProgress).await?;
        tracing::info!(
            "Task {} rejected in Human Review, reason: {}, returned to InProgress",
            task_id,
            reason
        );

        // TODO: Create a rejection feedback task or store the reason

        if let Some(publisher) = share_publisher
            && let Err(err) = publisher.update_shared_task_by_id(task_id).await
        {
            tracing::warn!(
                ?err,
                "Failed to propagate shared task update for {}",
                task_id
            );
        }

        Ok(TaskStatus::InProgress)
    }

    /// Get workflow configuration for a project.
    /// Returns default config if project has no workflow_config set.
    async fn get_workflow_config(
        &self,
        project_id: Uuid,
    ) -> Result<super::config::ProjectWorkflowConfig, ContainerError> {
        // Load project and extract workflow config
        let project = Project::find_by_id(&self.db().pool, project_id)
            .await?
            .ok_or_else(|| ContainerError::Other(anyhow!("Project not found: {}", project_id)))?;

        let db_config = project.get_workflow_config();
        Ok(super::config::ProjectWorkflowConfig {
            enable_human_review: db_config.enable_human_review,
            max_ai_review_iterations: db_config.max_ai_review_iterations,
            testing_requires_manual_exit: db_config.testing_requires_manual_exit,
            auto_start_ai_review: db_config.auto_start_ai_review,
            ai_review_prompt_template: db_config.ai_review_prompt_template,
        })
    }
}

/// Result of an AI review operation.
#[derive(Debug, Clone, PartialEq)]
pub enum AIReviewResult {
    /// AI review passed, task can proceed
    Pass,
    /// AI review failed, task needs revisions
    Fail { issues: Vec<String> },
    /// AI cannot determine outcome, needs human intervention
    NeedsIntervention,
}

#[cfg(test)]
mod status_transition_tests {
    use super::*;

    // Dummy struct to implement ContainerService for testing
    struct TestContainerService;

    #[async_trait::async_trait]
    impl ContainerService for TestContainerService {
        fn msg_stores(&self) -> &Arc<RwLock<HashMap<Uuid, Arc<MsgStore>>>> {
            unimplemented!()
        }

        fn db(&self) -> &DBService {
            unimplemented!()
        }

        fn git(&self) -> &GitService {
            unimplemented!()
        }

        fn share_publisher(&self) -> Option<&SharePublisher> {
            None
        }

        fn notification_service(&self) -> &NotificationService {
            unimplemented!()
        }

        fn workspace_to_current_dir(&self, _workspace: &Workspace) -> PathBuf {
            unimplemented!()
        }

        async fn create(&self, _workspace: &Workspace) -> Result<ContainerRef, ContainerError> {
            unimplemented!()
        }

        async fn kill_all_running_processes(&self) -> Result<(), ContainerError> {
            unimplemented!()
        }

        async fn delete(&self, _workspace: &Workspace) -> Result<(), ContainerError> {
            unimplemented!()
        }

        async fn ensure_container_exists(
            &self,
            _workspace: &Workspace,
        ) -> Result<ContainerRef, ContainerError> {
            unimplemented!()
        }

        async fn is_container_clean(&self, _workspace: &Workspace) -> Result<bool, ContainerError> {
            unimplemented!()
        }

        async fn start_execution_inner(
            &self,
            _workspace: &Workspace,
            _execution_process: &ExecutionProcess,
            _executor_action: &ExecutorAction,
        ) -> Result<(), ContainerError> {
            unimplemented!()
        }

        async fn stop_execution(
            &self,
            _execution_process: &ExecutionProcess,
            _status: ExecutionProcessStatus,
        ) -> Result<(), ContainerError> {
            unimplemented!()
        }

        async fn try_commit_changes(&self, _ctx: &ExecutionContext) -> Result<bool, ContainerError> {
            unimplemented!()
        }

        async fn copy_project_files(
            &self,
            _source_dir: &Path,
            _target_dir: &Path,
            _copy_files: &str,
        ) -> Result<(), ContainerError> {
            unimplemented!()
        }

        async fn stream_diff(
            &self,
            _workspace: &Workspace,
            _stats_only: bool,
        ) -> Result<futures::stream::BoxStream<'static, Result<LogMsg, std::io::Error>>, ContainerError> {
            unimplemented!()
        }

        async fn git_branch_prefix(&self) -> String {
            unimplemented!()
        }
    }

    fn create_test_service() -> TestContainerService {
        TestContainerService
    }

    #[tokio::test]
    async fn test_valid_status_transitions() {
        let service = create_test_service();
        use TaskStatus::*;

        // Default config (Human Review disabled, Testing requires manual exit)
        let default_config = Some(super::super::config::ProjectWorkflowConfig {
            enable_human_review: false,
            max_ai_review_iterations: 3,
            testing_requires_manual_exit: true,
            auto_start_ai_review: true,
            ai_review_prompt_template: None,
        });

        // Config with Human Review enabled
        let human_review_enabled = Some(super::super::config::ProjectWorkflowConfig {
            enable_human_review: true,
            max_ai_review_iterations: 3,
            testing_requires_manual_exit: true,
            auto_start_ai_review: true,
            ai_review_prompt_template: None,
        });

        // Happy path: InProgress -> Testing -> InReview -> Done
        assert!(service.validate_status_transition(Todo, InProgress, None).is_ok());
        assert!(service.validate_status_transition(InProgress, Testing, None).is_ok());
        assert!(service.validate_status_transition(Testing, InReview, None).is_ok());
        assert!(service.validate_status_transition(InReview, Done, None).is_ok());

        // Human Review path (requires config with enable_human_review = true)
        assert!(service.validate_status_transition(InReview, HumanReview, human_review_enabled.as_ref()).is_ok());
        assert!(service.validate_status_transition(HumanReview, Done, None).is_ok());

        // Testing can return to InProgress for revisions
        assert!(service.validate_status_transition(Testing, InProgress, None).is_ok());

        // Human Review can be rejected back to InProgress
        assert!(service.validate_status_transition(HumanReview, InProgress, None).is_ok());

        // Cancelling from various states
        assert!(service.validate_status_transition(InProgress, Cancelled, None).is_ok());
        assert!(service.validate_status_transition(Testing, Cancelled, None).is_ok());
        assert!(service.validate_status_transition(InReview, Cancelled, None).is_ok());
        assert!(service.validate_status_transition(HumanReview, Cancelled, None).is_ok());

        // Self-transitions (no-op) are allowed
        assert!(service.validate_status_transition(InProgress, InProgress, None).is_ok());
        assert!(service.validate_status_transition(Testing, Testing, None).is_ok());
    }

    #[tokio::test]
    async fn test_invalid_status_transitions() {
        let service = create_test_service();
        use TaskStatus::*;

        // Cannot start from Todo (except to InProgress)
        assert!(service.validate_status_transition(Todo, Testing, None).is_err());
        assert!(service.validate_status_transition(Todo, InReview, None).is_err());
        assert!(service.validate_status_transition(Todo, Done, None).is_err());

        // Cannot go to Todo
        assert!(service.validate_status_transition(InProgress, Todo, None).is_err());
        assert!(service.validate_status_transition(Testing, Todo, None).is_err());

        // Only InProgress can enter Testing
        assert!(service.validate_status_transition(Todo, Testing, None).is_err());
        assert!(service.validate_status_transition(InReview, Testing, None).is_err());

        // Cannot transition from Done
        assert!(service.validate_status_transition(Done, InProgress, None).is_err());
        assert!(service.validate_status_transition(Done, Testing, None).is_err());
        assert!(service.validate_status_transition(Done, InReview, None).is_err());

        // Cannot transition from Cancelled
        assert!(service.validate_status_transition(Cancelled, InProgress, None).is_err());
        assert!(service.validate_status_transition(Cancelled, Testing, None).is_err());

        // InReview cannot go back to InProgress directly
        assert!(service.validate_status_transition(InReview, InProgress, None).is_err());

        // InReview cannot go back to Testing
        assert!(service.validate_status_transition(InReview, Testing, None).is_err());

        // Only InReview can enter Human Review
        assert!(service.validate_status_transition(Testing, HumanReview, None).is_err());
        assert!(service.validate_status_transition(InProgress, HumanReview, None).is_err());
    }

    #[tokio::test]
    async fn test_ai_review_result_enum() {
        let pass = AIReviewResult::Pass;
        let fail = AIReviewResult::Fail {
            issues: vec!["Issue 1".to_string(), "Issue 2".to_string()],
        };
        let intervention = AIReviewResult::NeedsIntervention;

        assert_eq!(pass, AIReviewResult::Pass);
        assert!(matches!(fail, AIReviewResult::Fail { issues } if issues.len() == 2));
        assert!(matches!(intervention, AIReviewResult::NeedsIntervention));
    }

    #[tokio::test]
    async fn test_workflow_config_defaults() {
        use super::super::config::WorkflowConfig;

        let config = WorkflowConfig::default();

        assert!(!config.enable_human_review);
        assert_eq!(config.max_ai_review_iterations, 3);
        assert!(config.testing_requires_manual_exit);
        assert!(config.auto_start_ai_review);
        assert!(config.ai_review_prompt_template.is_none());
    }

    #[tokio::test]
    async fn test_config_aware_human_review() {
        let service = create_test_service();
        use TaskStatus::*;

        let human_review_enabled = Some(super::super::config::ProjectWorkflowConfig {
            enable_human_review: true,
            max_ai_review_iterations: 3,
            testing_requires_manual_exit: true,
            auto_start_ai_review: true,
            ai_review_prompt_template: None,
        });

        let human_review_disabled = Some(super::super::config::ProjectWorkflowConfig {
            enable_human_review: false,
            max_ai_review_iterations: 3,
            testing_requires_manual_exit: true,
            auto_start_ai_review: true,
            ai_review_prompt_template: None,
        });

        // Human Review enabled: InReview -> HumanReview should succeed
        assert!(service.validate_status_transition(InReview, HumanReview, human_review_enabled.as_ref()).is_ok());

        // Human Review disabled: InReview -> HumanReview should fail
        assert!(service.validate_status_transition(InReview, HumanReview, human_review_disabled.as_ref()).is_err());

        // With None config (backward compat): Human Review disabled by default
        assert!(service.validate_status_transition(InReview, HumanReview, None).is_err());
    }

    #[tokio::test]
    async fn test_config_aware_testing_bypass() {
        let service = create_test_service();
        use TaskStatus::*;

        let testing_bypass_allowed = Some(super::super::config::ProjectWorkflowConfig {
            enable_human_review: false,
            max_ai_review_iterations: 3,
            testing_requires_manual_exit: false, // Bypass allowed
            auto_start_ai_review: true,
            ai_review_prompt_template: None,
        });

        let testing_bypass_blocked = Some(super::super::config::ProjectWorkflowConfig {
            enable_human_review: false,
            max_ai_review_iterations: 3,
            testing_requires_manual_exit: true, // Bypass blocked
            auto_start_ai_review: true,
            ai_review_prompt_template: None,
        });

        // Testing bypass allowed: InProgress -> InReview should succeed
        assert!(service.validate_status_transition(InProgress, InReview, testing_bypass_allowed.as_ref()).is_ok());

        // Testing bypass blocked: InProgress -> InReview should fail
        assert!(service.validate_status_transition(InProgress, InReview, testing_bypass_blocked.as_ref()).is_err());

        // With None config (backward compat): Testing requires manual exit by default
        assert!(service.validate_status_transition(InProgress, InReview, None).is_err());
    }
}
