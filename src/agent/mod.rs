use crate::config::AppConfig;
use crate::indexer::RepoMap;
use crate::llm::client::LlmClient;
use crate::llm::prompt::{
    build_initial_prompt, build_repair_prompt, parse_explanation, parse_file_patches, FilePatch,
    SYSTEM_PROMPT,
};
use crate::verifier::patcher::{read_target_files, PatchTransaction};
use crate::verifier::runner::run_verification;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{info, warn};

#[derive(Debug, Clone, Deserialize)]
pub struct AgentTask {
    pub prompt: String,
    pub target_files: Vec<String>,
    pub workspace_root: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentResult {
    pub success: bool,
    pub attempts_used: usize,
    pub explanation: String,
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
            .unwrap_or_else(|| self.cfg.workspace_root.clone());

        info!("Building AST RepoMap for target project {:?}...", root);
        let repo_map = RepoMap::build(&root)?;
        let active_files = read_target_files(&root, &task.target_files);

        let initial_prompt = build_initial_prompt(&repo_map.rendered, &active_files, &task.prompt);
        let mut current_prompt = initial_prompt.clone();

        let mut tx = PatchTransaction::new(root.clone());
        let mut last_error: Option<String> = None;
        let mut last_explanation = String::new();

        for attempt in 1..=self.cfg.max_repair_attempts {
            info!(
                "Agent generation attempt {}/{}...",
                attempt, self.cfg.max_repair_attempts
            );

            let llm_output = self.llm.generate(SYSTEM_PROMPT, &current_prompt).await?;
            last_explanation = parse_explanation(&llm_output);
            let patches = parse_file_patches(&llm_output);

            if patches.is_empty() {
                warn!(
                    "LLM returned no valid === FILE: ... === blocks on attempt {}",
                    attempt
                );
                last_error = Some("Model did not return any valid file blocks.".to_string());
                current_prompt = build_repair_prompt(
                    &initial_prompt,
                    &llm_output,
                    "ERROR: You did not output any code using the required === FILE: path === ... === END FILE === format.",
                );
                continue;
            }

            tx.apply(&patches)?;

            let verification = run_verification(&root).await?;
            if verification.success {
                info!(
                    "Verification passed on attempt {}! Rolling back working tree so user can Apply in UI.",
                    attempt
                );
                tx.rollback()?;

                let modified_files = patches
                    .iter()
                    .map(|p| p.relative_path.to_string_lossy().to_string())
                    .collect();

                return Ok(AgentResult {
                    success: true,
                    attempts_used: attempt,
                    explanation: last_explanation,
                    proposed_patches: patches,
                    modified_files,
                    last_error: None,
                });
            } 

            warn!(
                "Verification failed on attempt {}. Feeding errors back to LLM...",
                attempt
            );
            last_error = Some(verification.error_log.clone());
            current_prompt =
                build_repair_prompt(&initial_prompt, &llm_output, &verification.error_log);
        }

        warn!("All attempts exhausted. Rolling back file changes...");
        if let Err(e) = tx.rollback() {
            bail!(
                "Failed to rollback patches after unsuccessful verification: {}",
                e
            );
        }

        Ok(AgentResult {
            success: false,
            attempts_used: self.cfg.max_repair_attempts,
            explanation: last_explanation,
            proposed_patches: Vec::new(),
            modified_files: Vec::new(),
            last_error,
        })
    }
}