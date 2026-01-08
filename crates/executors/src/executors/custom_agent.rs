use std::{path::Path, process::Stdio, sync::Arc};

use async_trait::async_trait;
use command_group::AsyncCommandGroup;
use derivative::Derivative;
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use tokio::{io::AsyncWriteExt, process::Command};
use ts_rs::TS;
use workspace_utils::msg_store::MsgStore;

use crate::{
    approvals::ExecutorApprovalService,
    command::{apply_overrides, CommandBuilder, CmdOverrides},
    env::ExecutionEnv,
    executors::{
        AppendPrompt, AvailabilityInfo, CodingAgent, ExecutorError, SpawnedChild,
        StandardCodingAgentExecutor,
        claude::{client::ClaudeAgentClient, protocol::ProtocolPeer, ClaudeLogProcessor, HistoryStrategy},
        codex::client::LogWriter,
    },
    logs::{stderr_processor::normalize_stderr_logs, utils::EntryIndexProvider},
    stdout_dup::create_stdout_pipe_writer,
};
use workspace_utils::shell::resolve_executable_path_blocking;

/// Enum for selecting the base agent type in the schema
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BaseAgentType {
    ClaudeCode,
    Amp,
    Gemini,
    Codex,
    Opencode,
    CursorAgent,
    QwenCode,
    Copilot,
    Droid,
}

/// Schema-only struct for base_agent that generates a cleaner form
/// This is only used for JSON Schema generation, not for actual data
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "CustomAgentBaseAgent")]
#[allow(non_snake_case)]
pub struct CustomAgentBaseAgentSchema {
    /// The type of base agent (for dropdown selection)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_field: Option<BaseAgentType>,
    /// ClaudeCode specific settings
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub CLAUDE_CODE: Option<super::claude::ClaudeCode>,
    /// AMP specific settings
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub AMP: Option<super::amp::Amp>,
    /// Gemini specific settings
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub GEMINI: Option<super::gemini::Gemini>,
    /// Codex specific settings
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub CODEX: Option<super::codex::Codex>,
    /// Opencode specific settings
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub OPENCODE: Option<super::opencode::Opencode>,
    /// CursorAgent specific settings
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub CURSOR_AGENT: Option<super::cursor::CursorAgent>,
    /// QwenCode specific settings
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub QWEN_CODE: Option<super::qwen::QwenCode>,
    /// Copilot specific settings
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub COPILOT: Option<super::copilot::Copilot>,
    /// Droid specific settings
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub DROID: Option<super::droid::Droid>,
}

/// Deserializer that validates base_agent is not a CustomAgent (prevents recursion)
fn deserialize_base_agent<'de, D>(deserializer: D) -> Result<Option<Box<CodingAgent>>, D::Error>
where
    D: Deserializer<'de>,
{
    let agent: Option<Box<CodingAgent>> = Option::deserialize(deserializer)?;
    if let Some(ref boxed) = agent {
        if matches!(boxed.as_ref(), CodingAgent::CustomAgent(_)) {
            return Err(serde::de::Error::custom(
                "Custom agent cannot be based on another Custom agent",
            ));
        }
    }
    Ok(agent)
}

/// User-defined custom agent that wraps another agent type with custom command
#[derive(Derivative, Clone, Serialize, Deserialize, TS, JsonSchema)]
#[derivative(Debug, PartialEq)]
pub struct CustomAgent {
    /// Display name for this custom agent
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Which agent type to base this custom agent on
    /// Uses CustomAgentBaseAgentSchema for schema generation to provide better form UX
    #[serde(
        alias = "baseAgent",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_base_agent"
    )]
    #[schemars(with = "CustomAgentBaseAgentSchema")]
    pub base_agent: Option<Box<CodingAgent>>,

    #[serde(default)]
    pub append_prompt: AppendPrompt,

    #[serde(flatten)]
    pub cmd: CmdOverrides,

    #[serde(skip)]
    #[ts(skip)]
    #[derivative(Debug = "ignore", PartialEq = "ignore")]
    approvals_service: Option<Arc<dyn ExecutorApprovalService>>,
}

impl CustomAgent {
    fn default_base_command(&self) -> &'static str {
        match self.base_agent.as_deref() {
            Some(CodingAgent::ClaudeCode(_)) => "npx -y @anthropic-ai/claude-code@2.0.76",
            Some(CodingAgent::Amp(_)) => "npx -y @sourcegraph/amp@0.0.1764777697-g907e30",
            Some(CodingAgent::Gemini(_)) => "npx -y @google/gemini-cli@latest",
            Some(CodingAgent::Codex(_)) => "npx -y @openai/codex@latest",
            Some(CodingAgent::Opencode(_)) => "npx -y @opencodeai/codex@latest",
            Some(CodingAgent::CursorAgent(_)) => "npx -y @cursor cursor-agent",
            Some(CodingAgent::QwenCode(_)) => "npx -y @qwen/qwen-code@latest",
            Some(CodingAgent::Copilot(_)) => "npx -y @copilot/copilot-cli@latest",
            Some(CodingAgent::Droid(_)) => "npx -y @anthropic/droid@latest",
            Some(CodingAgent::CustomAgent(_)) => {
                // This case is prevented by deserialize_base_agent validation
                unreachable!("Custom agent cannot be based on Custom agent")
            }
            None => "npx -y @anthropic-ai/claude-code@2.0.76",
        }
    }

    /// Build the command builder for this custom agent
    /// Inherits required flags from the base agent type
    fn build_command_builder(&self) -> CommandBuilder {
        let base = self
            .cmd
            .base_command_override
            .clone()
            .unwrap_or_else(|| self.default_base_command().to_string());

        let mut builder = CommandBuilder::new(base);

        // Add required flags based on base agent type
        match self.base_agent.as_deref() {
            Some(CodingAgent::ClaudeCode(_)) | None => {
                // Claude Code requires these flags for proper operation
                builder = builder.extend_params(["-p"]);
                builder = builder.extend_params([
                    "--verbose",
                    "--output-format=stream-json",
                    "--input-format=stream-json",
                    "--include-partial-messages",
                    "--disallowedTools=AskUserQuestion",
                ]);
                // Inherit dangerously_skip_permissions from base agent if set
                let skip_perms = match self.base_agent.as_deref() {
                    Some(CodingAgent::ClaudeCode(c)) => c.dangerously_skip_permissions.unwrap_or(false),
                    None => true, // Default to skip permissions when no base agent specified
                    _ => false,
                };
                if skip_perms {
                    builder = builder.extend_params(["--dangerously-skip-permissions"]);
                }
                // Handle model override from base agent
                if let Some(CodingAgent::ClaudeCode(c)) = self.base_agent.as_deref() {
                    if let Some(model) = &c.model {
                        builder = builder.extend_params(["--model", model]);
                    }
                }
            }
            Some(CodingAgent::Amp(_)) => {
                // Amp uses similar flags to Claude
                builder = builder.extend_params([
                    "--output-format=stream-json",
                    "--verbose",
                ]);
            }
            Some(CodingAgent::Gemini(gemini)) => {
                if gemini.yolo.unwrap_or(false) {
                    builder = builder.extend_params(["--yolo"]);
                }
            }
            Some(CodingAgent::Codex(codex)) => {
                if let Some(sandbox) = &codex.sandbox {
                    builder = builder.extend_params(["--sandbox", sandbox.as_ref()]);
                }
            }
            Some(CodingAgent::Droid(droid)) => {
                use crate::executors::droid::Autonomy;
                builder = builder.extend_params(["--output-format", "stream-json"]);
                builder = match &droid.autonomy {
                    Autonomy::Normal => builder,
                    Autonomy::Low => builder.extend_params(["--auto", "low"]),
                    Autonomy::Medium => builder.extend_params(["--auto", "medium"]),
                    Autonomy::High => builder.extend_params(["--auto", "high"]),
                    Autonomy::SkipPermissionsUnsafe => {
                        builder.extend_params(["--skip-permissions-unsafe"])
                    }
                };
                if let Some(model) = &droid.model {
                    builder = builder.extend_params(["--model", model.as_str()]);
                }
            }
            Some(CodingAgent::CursorAgent(cursor)) => {
                if cursor.force.unwrap_or(false) {
                    builder = builder.extend_params(["--force"]);
                }
                if let Some(model) = &cursor.model {
                    builder = builder.extend_params(["--model", model]);
                }
            }
            Some(CodingAgent::QwenCode(qwen)) => {
                if qwen.yolo.unwrap_or(false) {
                    builder = builder.extend_params(["--yolo"]);
                }
            }
            Some(CodingAgent::Opencode(opencode)) => {
                if opencode.auto_approve {
                    builder = builder.extend_params(["--auto-approve"]);
                }
            }
            Some(CodingAgent::Copilot(copilot)) => {
                if copilot.allow_all_tools.unwrap_or(false) {
                    builder = builder.extend_params(["--allow-all-tools"]);
                }
            }
            Some(CodingAgent::CustomAgent(_)) => {
                unreachable!("Custom agent cannot be based on Custom agent")
            }
        }

        // Apply any additional overrides from the custom agent itself
        apply_overrides(builder, &self.cmd)
    }
}

#[async_trait]
impl StandardCodingAgentExecutor for CustomAgent {
    fn use_approvals(&mut self, approvals: Arc<dyn ExecutorApprovalService>) {
        self.approvals_service = Some(approvals);
    }

    async fn spawn(
        &self,
        current_dir: &Path,
        prompt: &str,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        // When base agent is ClaudeCode, use the control protocol
        if let Some(CodingAgent::ClaudeCode(base_claude_config)) = self.base_agent.as_deref() {
            // Clone the config so we can use it
            let claude_config = base_claude_config.clone();

            // Build the command for ClaudeCode
            let command_builder = self.build_command_builder();
            let command_parts = command_builder.build_initial()?;
            let (executable_path, args) = command_parts.into_resolved().await?;

            let mut command = Command::new(executable_path);
            command
                .kill_on_drop(true)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .current_dir(current_dir)
                .args(&args);

            env.clone()
                .with_profile(&self.cmd)
                .apply_to_command(&mut command);

            let mut child = command.group_spawn()?;

            // Use ClaudeCode's control protocol
            let child_stdout = child.inner().stdout.take().ok_or_else(|| {
                ExecutorError::Io(std::io::Error::other("Claude Code missing stdout"))
            })?;
            let child_stdin = child.inner().stdin.take().ok_or_else(|| {
                ExecutorError::Io(std::io::Error::other("Claude Code missing stdin"))
            })?;

            let new_stdout = create_stdout_pipe_writer(&mut child)?;
            let hooks = claude_config.get_hooks();
            let permission_mode = claude_config.permission_mode();

            // The approvals service will be set via use_approvals() on self before this is called
            let approvals_clone = self.approvals_service.clone();
            let (interrupt_tx, interrupt_rx) = tokio::sync::oneshot::channel::<()>();

            let prompt_clone = self.append_prompt.combine_prompt(prompt);

            tokio::spawn(async move {
                let log_writer = LogWriter::new(new_stdout);
                let client = ClaudeAgentClient::new(log_writer.clone(), approvals_clone);
                let protocol_peer =
                    ProtocolPeer::spawn(child_stdin, child_stdout, client.clone(), interrupt_rx);

                if let Err(e) = protocol_peer.initialize(hooks).await {
                    tracing::error!("Failed to initialize control protocol: {e}");
                    let _ = log_writer
                        .log_raw(&format!("Error: Failed to initialize - {e}"))
                        .await;
                    return;
                }

                if let Err(e) = protocol_peer.set_permission_mode(permission_mode).await {
                    tracing::warn!("Failed to set permission mode to {permission_mode}: {e}");
                }

                if let Err(e) = protocol_peer.send_user_message(prompt_clone).await {
                    tracing::error!("Failed to send prompt: {e}");
                    let _ = log_writer
                        .log_raw(&format!("Error: Failed to send prompt - {e}"))
                        .await;
                }
            });

            Ok(SpawnedChild {
                child,
                exit_signal: None,
                interrupt_sender: Some(interrupt_tx),
            })
        } else {
            // For non-ClaudeCode base agents, use simple stdin piping
            let command_builder = self.build_command_builder();
            let command_parts = command_builder.build_initial()?;
            let (executable_path, args) = command_parts.into_resolved().await?;

            let combined_prompt = self.append_prompt.combine_prompt(prompt);

            let mut command = Command::new(executable_path);
            command
                .kill_on_drop(true)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .current_dir(current_dir)
                .args(&args);

            env.clone()
                .with_profile(&self.cmd)
                .apply_to_command(&mut command);

            let mut child = command.group_spawn()?;

            // Feed the prompt in, then close the pipe so the agent sees EOF
            if let Some(mut stdin) = child.inner().stdin.take() {
                stdin.write_all(combined_prompt.as_bytes()).await?;
                stdin.shutdown().await?;
            }

            Ok(child.into())
        }
    }

    async fn spawn_follow_up(
        &self,
        current_dir: &Path,
        prompt: &str,
        session_id: &str,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        // When base agent is ClaudeCode, use control protocol
        if let Some(CodingAgent::ClaudeCode(base_claude_config)) = self.base_agent.as_deref() {
            // Clone the config so we can use it
            let claude_config = base_claude_config.clone();

            let command_builder = self.build_command_builder();
            let command_parts = command_builder.build_follow_up(&[
                "--resume".to_string(),
                session_id.to_string(),
            ])?;
            let (executable_path, args) = command_parts.into_resolved().await?;

            let mut command = Command::new(executable_path);
            command
                .kill_on_drop(true)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .current_dir(current_dir)
                .args(&args);

            env.clone()
                .with_profile(&self.cmd)
                .apply_to_command(&mut command);

            let mut child = command.group_spawn()?;

            let child_stdout = child.inner().stdout.take().ok_or_else(|| {
                ExecutorError::Io(std::io::Error::other("Claude Code missing stdout"))
            })?;
            let child_stdin = child.inner().stdin.take().ok_or_else(|| {
                ExecutorError::Io(std::io::Error::other("Claude Code missing stdin"))
            })?;

            let new_stdout = create_stdout_pipe_writer(&mut child)?;
            let hooks = claude_config.get_hooks();
            let permission_mode = claude_config.permission_mode();

            let approvals_clone = self.approvals_service.clone();
            let (interrupt_tx, interrupt_rx) = tokio::sync::oneshot::channel::<()>();

            let prompt_clone = self.append_prompt.combine_prompt(prompt);

            tokio::spawn(async move {
                let log_writer = LogWriter::new(new_stdout);
                let client = ClaudeAgentClient::new(log_writer.clone(), approvals_clone);
                let protocol_peer =
                    ProtocolPeer::spawn(child_stdin, child_stdout, client.clone(), interrupt_rx);

                if let Err(e) = protocol_peer.initialize(hooks).await {
                    tracing::error!("Failed to initialize control protocol: {e}");
                    let _ = log_writer
                        .log_raw(&format!("Error: Failed to initialize - {e}"))
                        .await;
                    return;
                }

                if let Err(e) = protocol_peer.set_permission_mode(permission_mode).await {
                    tracing::warn!("Failed to set permission mode to {permission_mode}: {e}");
                }

                if let Err(e) = protocol_peer.send_user_message(prompt_clone).await {
                    tracing::error!("Failed to send prompt: {e}");
                    let _ = log_writer
                        .log_raw(&format!("Error: Failed to send prompt - {e}"))
                        .await;
                }
            });

            Ok(SpawnedChild {
                child,
                exit_signal: None,
                interrupt_sender: Some(interrupt_tx),
            })
        } else {
            // For non-ClaudeCode base agents, use simple stdin piping
            let command_builder = self.build_command_builder();
            let command_parts = command_builder.build_follow_up(&[
                "--resume".to_string(),
                session_id.to_string(),
            ])?;
            let (executable_path, args) = command_parts.into_resolved().await?;

            let combined_prompt = self.append_prompt.combine_prompt(prompt);

            let mut command = Command::new(executable_path);
            command
                .kill_on_drop(true)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .current_dir(current_dir)
                .args(&args);

            env.clone()
                .with_profile(&self.cmd)
                .apply_to_command(&mut command);

            let mut child = command.group_spawn()?;

            if let Some(mut stdin) = child.inner().stdin.take() {
                stdin.write_all(combined_prompt.as_bytes()).await?;
                stdin.shutdown().await?;
            }

            Ok(child.into())
        }
    }

    fn normalize_logs(&self, msg_store: Arc<MsgStore>, current_dir: &Path) {
        let entry_index_provider = EntryIndexProvider::start_from(&msg_store);

        // Delegate to appropriate log processor based on base agent type
        match self.base_agent.as_deref() {
            Some(CodingAgent::ClaudeCode(_)) | Some(CodingAgent::Amp(_)) | None => {
                // Claude-style JSON output (default)
                ClaudeLogProcessor::process_logs(
                    msg_store.clone(),
                    current_dir,
                    entry_index_provider.clone(),
                    HistoryStrategy::Default,
                );
                normalize_stderr_logs(msg_store, entry_index_provider);
            }
            Some(CodingAgent::Gemini(_))
            | Some(CodingAgent::Opencode(_))
            | Some(CodingAgent::QwenCode(_)) => {
                crate::executors::acp::normalize_logs(msg_store, current_dir);
            }
            Some(CodingAgent::Codex(_)) => {
                crate::executors::codex::normalize_logs::normalize_logs(msg_store, current_dir);
            }
            Some(CodingAgent::Droid(_)) => {
                crate::executors::droid::normalize_logs::normalize_logs(
                    msg_store.clone(),
                    current_dir,
                    entry_index_provider.clone(),
                );
                normalize_stderr_logs(msg_store, entry_index_provider);
            }
            Some(CodingAgent::CursorAgent(_)) | Some(CodingAgent::Copilot(_)) => {
                // These use stderr-only normalization
                normalize_stderr_logs(msg_store, entry_index_provider);
            }
            Some(CodingAgent::CustomAgent(_)) => {
                // Prevented by deserialize_base_agent validation
                unreachable!("Custom agent cannot be based on Custom agent")
            }
        }
    }

    fn default_mcp_config_path(&self) -> Option<std::path::PathBuf> {
        self.base_agent
            .as_deref()
            .and_then(|a: &CodingAgent| a.default_mcp_config_path())
    }

    fn get_availability_info(&self) -> AvailabilityInfo {
        // Check if custom command exists
        if let Some(ref override_cmd) = self.cmd.base_command_override {
            // Try to resolve the executable
            let resolved = resolve_executable_path_blocking(override_cmd);
            if resolved.is_some() {
                return AvailabilityInfo::InstallationFound;
            }
        }

        // Fall back to base agent's availability
        self.base_agent
            .as_deref()
            .map(|a: &CodingAgent| a.get_availability_info())
            .unwrap_or(AvailabilityInfo::NotFound)
    }
}
