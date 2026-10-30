use crate::config::AppConfig;
use crate::indexer::RepoMap;
use crate::llm::client::LlmClient;
use crate::llm::prompt::{
    build_initial_prompt, build_plan_prompt, build_repair_prompt, parse_commands,
    parse_explanation, parse_file_patches, FilePatch, PLAN_SYSTEM_PROMPT, SYSTEM_PROMPT,
};
use crate::verifier::patcher::{read_target_files, PatchTransaction};
use crate::verifier::runner::{ensure_cargo_project, run_verification, run_workspace_commands};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tracing::{info, warn};
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum TaskMode {
    #[default]
    Plan,
    Execute,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgentTask {
    #[serde(default)]
    pub mode: TaskMode,
    pub prompt: String,
    pub target_files: Vec<String>,
    pub workspace_root: Option<PathBuf>,
    pub previous_plan: Option<String>,
    pub approved_plan: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentResult {
    pub mode: TaskMode,
    pub success: bool,
    pub attempts_used: usize,
    pub plan: Option<String>,
    pub explanation: String,
    pub executed_commands: Vec<String>,
    pub proposed_patches: Vec<FilePatch>,
    pub modified_files: Vec<String>,
    pub last_error: Option<String>,
}

pub struct AstraAgent {
    cfg: AppConfig,
    llm: LlmClient,
}

impl AstraAgent {
    pub fn new(cfg: AppConfig) -> Result<Self> {
        let llm = LlmClient::new(cfg.llm_api_url.clone(), cfg.llm_model.clone())?;
        Ok(Self { cfg, llm })
    }

   pub async fn execute(&self, task: AgentTask) -> Result<AgentResult> {
        let root = task
            .workspace_root
            .clone()
            .unwrap_or_else(|| self.cfg.workspace_root.clone());

        match task.mode {
            TaskMode::Plan => self.generate_plan(&root, task).await,
            TaskMode::Execute => self.execute_plan(&root, task).await,
        }
    }

    async fn generate_plan(&self, root: &Path, task: AgentTask) -> Result<AgentResult> {
        info!("Building AST RepoMap for planning in {:?}...", root);
        let repo_map = RepoMap::build(root)?;
        let active_files = read_target_files(root, &task.target_files);

        let plan_prompt = build_plan_prompt(
            &repo_map.rendered,
            &active_files,
            &task.prompt,
            task.previous_plan.as_deref(),
        );

        info!("Requesting architectural plan from Gemma 4...");
        let plan_text = self.llm.generate(PLAN_SYSTEM_PROMPT, &plan_prompt).await?;

        Ok(AgentResult {
            mode: TaskMode::Plan,
            success: true,
            attempts_used: 1,
            plan: Some(plan_text.trim().to_string()),
            explanation: String::new(),
            executed_commands: Vec::new(),
            proposed_patches: Vec::new(),
            modified_files: Vec::new(),
            last_error: None,
        })
    }

    async fn execute_plan(&self, root: &Path, task: AgentTask) -> Result<AgentResult> {
        ensure_cargo_project(root).await?;

        info!("Building AST RepoMap for autonomous build in {:?}...", root);
        let repo_map = RepoMap::build(root)?;
        let active_files = read_target_files(root, &task.target_files);

        let full_task_description = match &task.approved_plan {
            Some(plan) => format!(
                "ORIGINAL REQUEST:\n{}\n\nAPPROVED ARCHITECTURAL PLAN TO EXECUTE:\n{}",
                task.prompt, plan
            ),
            None => task.prompt.clone(),
        };

        let mut current_prompt =
            build_initial_prompt(&repo_map.rendered, &active_files, &full_task_description);

        let mut tx = PatchTransaction::new(root.to_path_buf());
        let mut accumulated_patches: BTreeMap<String, FilePatch> = BTreeMap::new();
        let mut last_error: Option<String> = None;
        let mut last_explanation = String::new();
        let mut all_executed_cmds: Vec<String> = Vec::new();

        for attempt in 1..=self.cfg.max_repair_attempts {
            info!(
                "Autonomous build & verification attempt {}/{}...",
                attempt, self.cfg.max_repair_attempts
            );

            let llm_output = self.llm.generate(SYSTEM_PROMPT, &current_prompt).await?;
            let exp = parse_explanation(&llm_output);
            if !exp.is_empty() {
                last_explanation = exp;
            }

            let commands = parse_commands(&llm_output);
            if !commands.is_empty() {
                let mut ran = run_workspace_commands(root, &commands).await?;
                all_executed_cmds.append(&mut ran);
            }

            let patches = parse_file_patches(&llm_output);

            if patches.is_empty() {
                warn!(
                    "LLM returned no valid === FILE: ... === blocks on attempt {}",
                    attempt
                );
                last_error = Some("Model did not return any valid file blocks.".to_string());
                let tracked_paths: Vec<String> = accumulated_patches.keys().cloned().collect();
                let disk_files = read_target_files(root, &tracked_paths);
                let updated_map = RepoMap::build(root).map(|m| m.rendered).unwrap_or_default();
                current_prompt = build_repair_prompt(
                    &full_task_description,
                    &updated_map,
                    &disk_files,
                    "ERROR: You did not output any code using the required === FILE: path === ... === END FILE === format.",
                );
                continue;
            }

            tx.apply(&patches)?;
            for p in patches {
                let key = p.relative_path.to_string_lossy().to_string();
                accumulated_patches.insert(key, p);
            }

            let verification = run_verification(root).await?;
            if verification.success {
                info!(
                    "Autonomous build verified by clippy & cargo test on attempt {}! Project files saved.",
                    attempt
                );

                // Перечитываем финальное содержимое файлов с диска (с учетом правок от cargo clippy --fix)
                let modified_files: Vec<String> = accumulated_patches.keys().cloned().collect();
                let final_patches = read_target_files(root, &modified_files)
                    .into_iter()
                    .map(|(rel, content)| FilePatch {
                        relative_path: PathBuf::from(rel),
                        content,
                    })
                    .collect();

                return Ok(AgentResult {
                    mode: TaskMode::Execute,
                    success: true,
                    attempts_used: attempt,
                    plan: task.approved_plan,
                    explanation: last_explanation,
                    executed_commands: all_executed_cmds,
                    proposed_patches: final_patches,
                    modified_files,
                    last_error: None,
                });
            }

            warn!(
                "Verification failed on attempt {}/{}. Feeding compiler errors back to Gemma 4...",
                attempt, self.cfg.max_repair_attempts
            );
            last_error = Some(verification.error_log.clone());

            let tracked_paths: Vec<String> = accumulated_patches.keys().cloned().collect();
            let disk_files = read_target_files(root, &tracked_paths);
            let updated_map = RepoMap::build(root).map(|m| m.rendered).unwrap_or_default();

            current_prompt = build_repair_prompt(
                &full_task_description,
                &updated_map,
                &disk_files,
                &verification.error_log,
            );
        }

        warn!("All {} build attempts exhausted. Keeping generated files on disk so user can inspect or continue.", self.cfg.max_repair_attempts);
        let modified_files: Vec<String> = accumulated_patches.keys().cloned().collect();
        let final_patches: Vec<FilePatch> = accumulated_patches.into_values().collect();

        Ok(AgentResult {
            mode: TaskMode::Execute,
            success: false,
            attempts_used: self.cfg.max_repair_attempts,
            plan: task.approved_plan,
            explanation: last_explanation,
            executed_commands: all_executed_cmds,
            proposed_patches: final_patches,
            modified_files,
            last_error,
        })
    }
}