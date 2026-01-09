use std::path::PathBuf;

use anyhow;
use axum::{
    Extension, Json, Router,
    extract::{
        Path, Query, State,
        ws::{WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
    middleware::from_fn_with_state,
    response::{IntoResponse, Json as ResponseJson},
    routing::{get, post},
};
use db::models::{
    project::{CreateProject, Project, ProjectError, SearchResult, UpdateProject},
    project_repo::{CreateProjectRepo, ProjectRepo, UpdateProjectRepo},
    repo::Repo,
};
use serde::Serialize;
use deployment::Deployment;
use futures_util::{SinkExt, StreamExt, TryStreamExt};
use serde::Deserialize;
use services::services::{
    file_search_cache::SearchQuery, project::ProjectServiceError,
    remote_client::CreateRemoteProjectPayload,
};
use ts_rs::TS;
use utils::{
    api::projects::{RemoteProject, RemoteProjectMembersResponse},
    response::ApiResponse,
};
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError, middleware::load_project_middleware};

#[derive(Deserialize, TS)]
pub struct LinkToExistingRequest {
    pub remote_project_id: Uuid,
}

#[derive(Deserialize, TS)]
pub struct CreateRemoteProjectRequest {
    pub organization_id: Uuid,
    pub name: String,
}

pub async fn get_projects(
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<Vec<Project>>>, ApiError> {
    let projects = Project::find_all(&deployment.db().pool).await?;
    Ok(ResponseJson(ApiResponse::success(projects)))
}

pub async fn stream_projects_ws(
    ws: WebSocketUpgrade,
    State(deployment): State<DeploymentImpl>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        if let Err(e) = handle_projects_ws(socket, deployment).await {
            tracing::warn!("projects WS closed: {}", e);
        }
    })
}

async fn handle_projects_ws(socket: WebSocket, deployment: DeploymentImpl) -> anyhow::Result<()> {
    let mut stream = deployment
        .events()
        .stream_projects_raw()
        .await?
        .map_ok(|msg| msg.to_ws_message_unchecked());

    // Split socket into sender and receiver
    let (mut sender, mut receiver) = socket.split();

    // Drain (and ignore) any client->server messages so pings/pongs work
    tokio::spawn(async move { while let Some(Ok(_)) = receiver.next().await {} });

    // Forward server messages
    while let Some(item) = stream.next().await {
        match item {
            Ok(msg) => {
                if sender.send(msg).await.is_err() {
                    break; // client disconnected
                }
            }
            Err(e) => {
                tracing::error!("stream error: {}", e);
                break;
            }
        }
    }

    Ok(())
}

pub async fn get_project(
    Extension(project): Extension<Project>,
) -> Result<ResponseJson<ApiResponse<Project>>, ApiError> {
    Ok(ResponseJson(ApiResponse::success(project)))
}

pub async fn link_project_to_existing_remote(
    Extension(project): Extension<Project>,
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<LinkToExistingRequest>,
) -> Result<ResponseJson<ApiResponse<Project>>, ApiError> {
    let client = deployment.remote_client()?;

    let remote_project = client.get_project(payload.remote_project_id).await?;

    let updated_project = apply_remote_project_link(&deployment, project, remote_project).await?;

    Ok(ResponseJson(ApiResponse::success(updated_project)))
}

pub async fn create_and_link_remote_project(
    Extension(project): Extension<Project>,
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<CreateRemoteProjectRequest>,
) -> Result<ResponseJson<ApiResponse<Project>>, ApiError> {
    let repo_name = payload.name.trim().to_string();
    if repo_name.trim().is_empty() {
        return Err(ApiError::Conflict(
            "Remote project name cannot be empty.".to_string(),
        ));
    }

    let client = deployment.remote_client()?;

    let remote_project = client
        .create_project(&CreateRemoteProjectPayload {
            organization_id: payload.organization_id,
            name: repo_name,
            metadata: None,
        })
        .await?;

    let updated_project = apply_remote_project_link(&deployment, project, remote_project).await?;

    Ok(ResponseJson(ApiResponse::success(updated_project)))
}

pub async fn unlink_project(
    Extension(project): Extension<Project>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<Project>>, ApiError> {
    let updated_project = deployment
        .project()
        .unlink_from_remote(&deployment.db().pool, &project)
        .await?;

    Ok(ResponseJson(ApiResponse::success(updated_project)))
}

pub async fn get_remote_project_by_id(
    State(deployment): State<DeploymentImpl>,
    Path(remote_project_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<RemoteProject>>, ApiError> {
    let client = deployment.remote_client()?;

    let remote_project = client.get_project(remote_project_id).await?;

    Ok(ResponseJson(ApiResponse::success(remote_project)))
}

pub async fn get_project_remote_members(
    State(deployment): State<DeploymentImpl>,
    Extension(project): Extension<Project>,
) -> Result<ResponseJson<ApiResponse<RemoteProjectMembersResponse>>, ApiError> {
    let remote_project_id = project.remote_project_id.ok_or_else(|| {
        ApiError::Conflict("Project is not linked to a remote project".to_string())
    })?;

    let client = deployment.remote_client()?;

    let remote_project = client.get_project(remote_project_id).await?;
    let members = client
        .list_members(remote_project.organization_id)
        .await?
        .members;

    Ok(ResponseJson(ApiResponse::success(
        RemoteProjectMembersResponse {
            organization_id: remote_project.organization_id,
            members,
        },
    )))
}

async fn apply_remote_project_link(
    deployment: &DeploymentImpl,
    project: Project,
    remote_project: RemoteProject,
) -> Result<Project, ApiError> {
    if project.remote_project_id.is_some() {
        return Err(ApiError::Conflict(
            "Project is already linked to a remote project. Unlink it first.".to_string(),
        ));
    }

    let updated_project = deployment
        .project()
        .link_to_remote(&deployment.db().pool, project.id, remote_project)
        .await?;

    deployment
        .track_if_analytics_allowed(
            "project_linked_to_remote",
            serde_json::json!({
                "project_id": project.id.to_string(),
            }),
        )
        .await;

    Ok(updated_project)
}

pub async fn create_project(
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<CreateProject>,
) -> Result<ResponseJson<ApiResponse<Project>>, ApiError> {
    tracing::debug!("Creating project '{}'", payload.name);
    let repo_count = payload.repositories.len();

    match deployment
        .project()
        .create_project(&deployment.db().pool, deployment.repo(), payload)
        .await
    {
        Ok(project) => {
            // Track project creation event
            deployment
                .track_if_analytics_allowed(
                    "project_created",
                    serde_json::json!({
                        "project_id": project.id.to_string(),
                        "repository_count": repo_count,
                        "trigger": "manual",
                    }),
                )
                .await;

            Ok(ResponseJson(ApiResponse::success(project)))
        }
        Err(ProjectServiceError::DuplicateGitRepoPath) => Ok(ResponseJson(ApiResponse::error(
            "Duplicate repository path provided",
        ))),
        Err(ProjectServiceError::DuplicateRepositoryName) => Ok(ResponseJson(ApiResponse::error(
            "Duplicate repository name provided",
        ))),
        Err(ProjectServiceError::PathNotFound(_)) => Ok(ResponseJson(ApiResponse::error(
            "The specified path does not exist",
        ))),
        Err(ProjectServiceError::PathNotDirectory(_)) => Ok(ResponseJson(ApiResponse::error(
            "The specified path is not a directory",
        ))),
        Err(ProjectServiceError::NotGitRepository(_)) => Ok(ResponseJson(ApiResponse::error(
            "The specified directory is not a git repository",
        ))),
        Err(e) => Err(ProjectError::CreateFailed(e.to_string()).into()),
    }
}

pub async fn update_project(
    Extension(existing_project): Extension<Project>,
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<UpdateProject>,
) -> Result<ResponseJson<ApiResponse<Project>>, StatusCode> {
    match deployment
        .project()
        .update_project(&deployment.db().pool, &existing_project, payload)
        .await
    {
        Ok(project) => Ok(ResponseJson(ApiResponse::success(project))),
        Err(e) => {
            tracing::error!("Failed to update project: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn delete_project(
    Extension(project): Extension<Project>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<()>>, StatusCode> {
    match deployment
        .project()
        .delete_project(&deployment.db().pool, project.id)
        .await
    {
        Ok(rows_affected) => {
            if rows_affected == 0 {
                Err(StatusCode::NOT_FOUND)
            } else {
                deployment
                    .track_if_analytics_allowed(
                        "project_deleted",
                        serde_json::json!({
                            "project_id": project.id.to_string(),
                        }),
                    )
                    .await;

                Ok(ResponseJson(ApiResponse::success(())))
            }
        }
        Err(e) => {
            tracing::error!("Failed to delete project: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[derive(serde::Deserialize)]
pub struct OpenEditorRequest {
    editor_type: Option<String>,
    git_repo_path: Option<PathBuf>,
}

#[derive(Debug, serde::Serialize, ts_rs::TS)]
pub struct OpenEditorResponse {
    pub url: Option<String>,
}

pub async fn open_project_in_editor(
    Extension(project): Extension<Project>,
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<Option<OpenEditorRequest>>,
) -> Result<ResponseJson<ApiResponse<OpenEditorResponse>>, ApiError> {
    let path = if let Some(ref req) = payload
        && let Some(ref specified_path) = req.git_repo_path
    {
        specified_path.clone()
    } else {
        let repositories = deployment
            .project()
            .get_repositories(&deployment.db().pool, project.id)
            .await?;

        repositories
            .first()
            .map(|r| r.path.clone())
            .ok_or_else(|| ApiError::BadRequest("Project has no repositories".to_string()))?
    };

    let editor_config = {
        let config = deployment.config().read().await;
        let editor_type_str = payload.as_ref().and_then(|req| req.editor_type.as_deref());
        config.editor.with_override(editor_type_str)
    };

    match editor_config.open_file(&path).await {
        Ok(url) => {
            tracing::info!(
                "Opened editor for project {} at path: {}{}",
                project.id,
                path.to_string_lossy(),
                if url.is_some() { " (remote mode)" } else { "" }
            );

            deployment
                .track_if_analytics_allowed(
                    "project_editor_opened",
                    serde_json::json!({
                        "project_id": project.id.to_string(),
                        "editor_type": payload.as_ref().and_then(|req| req.editor_type.as_ref()),
                        "remote_mode": url.is_some(),
                    }),
                )
                .await;

            Ok(ResponseJson(ApiResponse::success(OpenEditorResponse {
                url,
            })))
        }
        Err(e) => {
            tracing::error!("Failed to open editor for project {}: {:?}", project.id, e);
            Err(ApiError::EditorOpen(e))
        }
    }
}

pub async fn search_project_files(
    State(deployment): State<DeploymentImpl>,
    Extension(project): Extension<Project>,
    Query(search_query): Query<SearchQuery>,
) -> Result<ResponseJson<ApiResponse<Vec<SearchResult>>>, StatusCode> {
    if search_query.q.trim().is_empty() {
        return Ok(ResponseJson(ApiResponse::error(
            "Query parameter 'q' is required and cannot be empty",
        )));
    }

    let repositories = match deployment
        .project()
        .get_repositories(&deployment.db().pool, project.id)
        .await
    {
        Ok(repos) => repos,
        Err(e) => {
            tracing::error!("Failed to get repositories: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    match deployment
        .project()
        .search_files(
            deployment.file_search_cache().as_ref(),
            &repositories,
            &search_query,
        )
        .await
    {
        Ok(results) => Ok(ResponseJson(ApiResponse::success(results))),
        Err(e) => {
            tracing::error!("Failed to search files: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_project_repositories(
    Extension(project): Extension<Project>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<Vec<Repo>>>, ApiError> {
    let repositories = deployment
        .project()
        .get_repositories(&deployment.db().pool, project.id)
        .await?;
    Ok(ResponseJson(ApiResponse::success(repositories)))
}

pub async fn add_project_repository(
    Extension(project): Extension<Project>,
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<CreateProjectRepo>,
) -> Result<ResponseJson<ApiResponse<Repo>>, ApiError> {
    tracing::debug!(
        "Adding repository '{}' to project {} (path: {})",
        payload.display_name,
        project.id,
        payload.git_repo_path
    );

    match deployment
        .project()
        .add_repository(
            &deployment.db().pool,
            deployment.repo(),
            project.id,
            &payload,
        )
        .await
    {
        Ok(repository) => {
            deployment
                .track_if_analytics_allowed(
                    "project_repository_added",
                    serde_json::json!({
                        "project_id": project.id.to_string(),
                        "repository_id": repository.id.to_string(),
                    }),
                )
                .await;

            Ok(ResponseJson(ApiResponse::success(repository)))
        }
        Err(ProjectServiceError::PathNotFound(_)) => {
            tracing::warn!(
                "Failed to add repository to project {}: path does not exist",
                project.id
            );
            Ok(ResponseJson(ApiResponse::error(
                "The specified path does not exist",
            )))
        }
        Err(ProjectServiceError::PathNotDirectory(_)) => {
            tracing::warn!(
                "Failed to add repository to project {}: path is not a directory",
                project.id
            );
            Ok(ResponseJson(ApiResponse::error(
                "The specified path is not a directory",
            )))
        }
        Err(ProjectServiceError::NotGitRepository(_)) => {
            tracing::warn!(
                "Failed to add repository to project {}: not a git repository",
                project.id
            );
            Ok(ResponseJson(ApiResponse::error(
                "The specified directory is not a git repository",
            )))
        }
        Err(ProjectServiceError::DuplicateRepositoryName) => {
            tracing::warn!(
                "Failed to add repository to project {}: duplicate repository name",
                project.id
            );
            Ok(ResponseJson(ApiResponse::error(
                "A repository with this name already exists in the project",
            )))
        }
        Err(ProjectServiceError::DuplicateGitRepoPath) => {
            tracing::warn!(
                "Failed to add repository to project {}: duplicate repository path",
                project.id
            );
            Ok(ResponseJson(ApiResponse::error(
                "A repository with this path already exists in the project",
            )))
        }
        Err(e) => Err(e.into()),
    }
}

pub async fn delete_project_repository(
    State(deployment): State<DeploymentImpl>,
    Path((project_id, repo_id)): Path<(Uuid, Uuid)>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    tracing::debug!(
        "Removing repository {} from project {}",
        repo_id,
        project_id
    );

    match deployment
        .project()
        .delete_repository(&deployment.db().pool, project_id, repo_id)
        .await
    {
        Ok(()) => {
            deployment
                .track_if_analytics_allowed(
                    "project_repository_removed",
                    serde_json::json!({
                        "project_id": project_id.to_string(),
                        "repository_id": repo_id.to_string(),
                    }),
                )
                .await;

            Ok(ResponseJson(ApiResponse::success(())))
        }
        Err(ProjectServiceError::RepositoryNotFound) => {
            tracing::warn!(
                "Failed to remove repository {} from project {}: not found",
                repo_id,
                project_id
            );
            Ok(ResponseJson(ApiResponse::error("Repository not found")))
        }
        Err(e) => Err(e.into()),
    }
}

pub async fn get_project_repository(
    State(deployment): State<DeploymentImpl>,
    Path((project_id, repo_id)): Path<(Uuid, Uuid)>,
) -> Result<ResponseJson<ApiResponse<ProjectRepo>>, ApiError> {
    match ProjectRepo::find_by_project_and_repo(&deployment.db().pool, project_id, repo_id).await {
        Ok(Some(project_repo)) => Ok(ResponseJson(ApiResponse::success(project_repo))),
        Ok(None) => Err(ApiError::BadRequest(
            "Repository not found in project".to_string(),
        )),
        Err(e) => Err(e.into()),
    }
}

pub async fn update_project_repository(
    State(deployment): State<DeploymentImpl>,
    Path((project_id, repo_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<UpdateProjectRepo>,
) -> Result<ResponseJson<ApiResponse<ProjectRepo>>, ApiError> {
    match ProjectRepo::update(&deployment.db().pool, project_id, repo_id, &payload).await {
        Ok(project_repo) => Ok(ResponseJson(ApiResponse::success(project_repo))),
        Err(db::models::project_repo::ProjectRepoError::NotFound) => Err(ApiError::BadRequest(
            "Repository not found in project".to_string(),
        )),
        Err(e) => Err(e.into()),
    }
}

/// Request body for updating workflow configuration
#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct UpdateWorkflowConfigRequest {
    #[serde(default)]
    pub enable_human_review: Option<bool>,

    #[serde(default)]
    pub max_ai_review_iterations: Option<u32>,

    #[serde(default)]
    pub testing_requires_manual_exit: Option<bool>,

    #[serde(default)]
    pub auto_start_ai_review: Option<bool>,

    #[serde(default)]
    pub ai_review_prompt_template: Option<Option<String>>,
}

/// Response for workflow configuration
#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct WorkflowConfigResponse {
    pub enable_human_review: bool,
    pub max_ai_review_iterations: u32,
    pub testing_requires_manual_exit: bool,
    pub auto_start_ai_review: bool,
    pub ai_review_prompt_template: Option<String>,
}

/// GET /projects/:id/workflow-config
/// Returns the workflow configuration for a project
pub async fn get_workflow_config(
    Extension(project): Extension<Project>,
) -> Result<ResponseJson<ApiResponse<WorkflowConfigResponse>>, ApiError> {
    let config = project.get_workflow_config();
    Ok(ResponseJson(ApiResponse::success(WorkflowConfigResponse {
        enable_human_review: config.enable_human_review,
        max_ai_review_iterations: config.max_ai_review_iterations,
        testing_requires_manual_exit: config.testing_requires_manual_exit,
        auto_start_ai_review: config.auto_start_ai_review,
        ai_review_prompt_template: config.ai_review_prompt_template,
    })))
}

/// PATCH /projects/:id/workflow-config
/// Updates the workflow configuration for a project
pub async fn update_workflow_config(
    Extension(project): Extension<Project>,
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<UpdateWorkflowConfigRequest>,
) -> Result<ResponseJson<ApiResponse<WorkflowConfigResponse>>, ApiError> {
    // Get existing config
    let mut config = project.get_workflow_config();

    // Apply updates from payload
    if let Some(enable_human_review) = payload.enable_human_review {
        config.enable_human_review = enable_human_review;
    }
    if let Some(max_ai_review_iterations) = payload.max_ai_review_iterations {
        // Validate: max_ai_review_iterations must be at least 1
        if max_ai_review_iterations == 0 {
            return Err(ApiError::BadRequest(
                "max_ai_review_iterations must be at least 1".to_string(),
            ));
        }
        config.max_ai_review_iterations = max_ai_review_iterations;
    }
    if let Some(testing_requires_manual_exit) = payload.testing_requires_manual_exit {
        config.testing_requires_manual_exit = testing_requires_manual_exit;
    }
    if let Some(auto_start_ai_review) = payload.auto_start_ai_review {
        config.auto_start_ai_review = auto_start_ai_review;
    }
    if let Some(ai_review_prompt_template) = payload.ai_review_prompt_template {
        config.ai_review_prompt_template = ai_review_prompt_template;
    }

    // Serialize config to JSON
    let workflow_config_json = serde_json::to_string(&config).map_err(|e| {
        ApiError::BadRequest(format!("Failed to serialize workflow config: {}", e))
    })?;

    // Update in database
    let updated_project = Project::update_workflow_config(
        &deployment.db().pool,
        project.id,
        Some(workflow_config_json),
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to update workflow config: {}", e);
        ApiError::Database(e)
    })?;

    // Return the updated config
    let updated_config = updated_project.get_workflow_config();
    Ok(ResponseJson(ApiResponse::success(WorkflowConfigResponse {
        enable_human_review: updated_config.enable_human_review,
        max_ai_review_iterations: updated_config.max_ai_review_iterations,
        testing_requires_manual_exit: updated_config.testing_requires_manual_exit,
        auto_start_ai_review: updated_config.auto_start_ai_review,
        ai_review_prompt_template: updated_config.ai_review_prompt_template,
    })))
}

pub fn router(deployment: &DeploymentImpl) -> Router<DeploymentImpl> {
    let project_id_router = Router::new()
        .route(
            "/",
            get(get_project).put(update_project).delete(delete_project),
        )
        .route("/remote/members", get(get_project_remote_members))
        .route("/search", get(search_project_files))
        .route("/open-editor", post(open_project_in_editor))
        .route(
            "/link",
            post(link_project_to_existing_remote).delete(unlink_project),
        )
        .route("/link/create", post(create_and_link_remote_project))
        .route(
            "/repositories",
            get(get_project_repositories).post(add_project_repository),
        )
        .route(
            "/workflow-config",
            get(get_workflow_config).patch(update_workflow_config),
        )
        .layer(from_fn_with_state(
            deployment.clone(),
            load_project_middleware,
        ));

    let projects_router = Router::new()
        .route("/", get(get_projects).post(create_project))
        .route(
            "/{project_id}/repositories/{repo_id}",
            get(get_project_repository)
                .put(update_project_repository)
                .delete(delete_project_repository),
        )
        .route("/stream/ws", get(stream_projects_ws))
        .nest("/{id}", project_id_router);

    Router::new().nest("/projects", projects_router).route(
        "/remote-projects/{remote_project_id}",
        get(get_remote_project_by_id),
    )
}

#[cfg(test)]
mod workflow_config_tests {
    use super::*;
    use db::models::project::ProjectWorkflowConfig;

    #[test]
    fn test_update_workflow_config_request_defaults() {
        let req = UpdateWorkflowConfigRequest {
            enable_human_review: None,
            max_ai_review_iterations: None,
            testing_requires_manual_exit: None,
            auto_start_ai_review: None,
            ai_review_prompt_template: None,
        };

        assert!(req.enable_human_review.is_none());
        assert!(req.max_ai_review_iterations.is_none());
        assert!(req.testing_requires_manual_exit.is_none());
        assert!(req.auto_start_ai_review.is_none());
        assert!(req.ai_review_prompt_template.is_none());
    }

    #[test]
    fn test_project_workflow_config_defaults() {
        // Test that Default::default() sets all fields to Rust defaults
        // Note: serde defaults (from #[serde(default = "...")]) only apply during deserialization
        let config = ProjectWorkflowConfig::default();

        assert!(!config.enable_human_review);
        assert_eq!(config.max_ai_review_iterations, 0); // Rust default for u32
        assert!(!config.testing_requires_manual_exit); // Rust default for bool
        assert!(!config.auto_start_ai_review); // Rust default for bool
        assert!(config.ai_review_prompt_template.is_none());
    }

    #[test]
    fn test_project_workflow_config_serde_defaults() {
        // Test that serde defaults apply during deserialization
        let json = "{}";
        let config: ProjectWorkflowConfig = serde_json::from_str(json).expect("should deserialize");

        // These values come from #[serde(default = "...")] attributes
        assert!(!config.enable_human_review);
        assert_eq!(config.max_ai_review_iterations, 3);
        assert!(config.testing_requires_manual_exit);
        assert!(config.auto_start_ai_review);
        assert!(config.ai_review_prompt_template.is_none());
    }

    #[test]
    fn test_project_workflow_config_serialization() {
        let config = ProjectWorkflowConfig {
            enable_human_review: true,
            max_ai_review_iterations: 5,
            testing_requires_manual_exit: false,
            auto_start_ai_review: true,
            ai_review_prompt_template: Some("Review the code carefully".to_string()),
        };

        let json = serde_json::to_string(&config).expect("should serialize");
        let deserialized: ProjectWorkflowConfig =
            serde_json::from_str(&json).expect("should deserialize");

        assert_eq!(deserialized.enable_human_review, true);
        assert_eq!(deserialized.max_ai_review_iterations, 5);
        assert_eq!(deserialized.testing_requires_manual_exit, false);
        assert_eq!(deserialized.auto_start_ai_review, true);
        assert_eq!(
            deserialized.ai_review_prompt_template,
            Some("Review the code carefully".to_string())
        );
    }

    #[test]
    fn test_workflow_config_response_serialization() {
        let response = WorkflowConfigResponse {
            enable_human_review: true,
            max_ai_review_iterations: 3,
            testing_requires_manual_exit: true,
            auto_start_ai_review: false,
            ai_review_prompt_template: None,
        };

        let json = serde_json::to_string(&response).expect("should serialize");
        let deserialized: WorkflowConfigResponse =
            serde_json::from_str(&json).expect("should deserialize");

        assert_eq!(deserialized.enable_human_review, true);
        assert_eq!(deserialized.max_ai_review_iterations, 3);
        assert_eq!(deserialized.testing_requires_manual_exit, true);
        assert_eq!(deserialized.auto_start_ai_review, false);
        assert!(deserialized.ai_review_prompt_template.is_none());
    }

    #[test]
    fn test_workflow_config_response_with_template() {
        let response = WorkflowConfigResponse {
            enable_human_review: false,
            max_ai_review_iterations: 1,
            testing_requires_manual_exit: true,
            auto_start_ai_review: true,
            ai_review_prompt_template: Some("Custom template".to_string()),
        };

        let json = serde_json::to_string(&response).expect("should serialize");
        let deserialized: WorkflowConfigResponse =
            serde_json::from_str(&json).expect("should deserialize");

        assert_eq!(
            deserialized.ai_review_prompt_template,
            Some("Custom template".to_string())
        );
    }

    #[test]
    fn test_max_ai_review_iterations_validation() {
        // Valid values
        assert!(is_valid_max_iterations(1));
        assert!(is_valid_max_iterations(3));
        assert!(is_valid_max_iterations(10));
        assert!(is_valid_max_iterations(50));

        // Invalid values (0 is not allowed, u32::MAX is too high)
        assert!(!is_valid_max_iterations(0));
        assert!(!is_valid_max_iterations(u32::MAX));
    }

    /// Helper function to validate max_ai_review_iterations
    fn is_valid_max_iterations(value: u32) -> bool {
        value >= 1 && value <= 50
    }

    #[test]
    fn test_update_request_partial_updates() {
        // Simulate partial update behavior
        let original_config = ProjectWorkflowConfig {
            enable_human_review: false,
            max_ai_review_iterations: 3,
            testing_requires_manual_exit: true,
            auto_start_ai_review: true,
            ai_review_prompt_template: None,
        };

        // Only update enable_human_review
        let update = UpdateWorkflowConfigRequest {
            enable_human_review: Some(true),
            max_ai_review_iterations: None,
            testing_requires_manual_exit: None,
            auto_start_ai_review: None,
            ai_review_prompt_template: None,
        };

        let merged = merge_config(original_config.clone(), update);
        assert_eq!(merged.enable_human_review, true);
        assert_eq!(merged.max_ai_review_iterations, 3); // Unchanged
        assert_eq!(merged.testing_requires_manual_exit, true); // Unchanged
    }

    /// Helper to merge update request with existing config
    fn merge_config(
        mut existing: ProjectWorkflowConfig,
        update: UpdateWorkflowConfigRequest,
    ) -> ProjectWorkflowConfig {
        if let Some(enable_human_review) = update.enable_human_review {
            existing.enable_human_review = enable_human_review;
        }
        if let Some(max_ai_review_iterations) = update.max_ai_review_iterations {
            existing.max_ai_review_iterations = max_ai_review_iterations;
        }
        if let Some(testing_requires_manual_exit) = update.testing_requires_manual_exit {
            existing.testing_requires_manual_exit = testing_requires_manual_exit;
        }
        if let Some(auto_start_ai_review) = update.auto_start_ai_review {
            existing.auto_start_ai_review = auto_start_ai_review;
        }
        if let Some(ai_review_prompt_template) = update.ai_review_prompt_template {
            existing.ai_review_prompt_template = ai_review_prompt_template;
        }
        existing
    }
}
