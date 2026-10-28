use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub llm_api_url: String,
    pub llm_model: String,
    pub workspace_root: PathBuf,
    pub max_repair_attempts: usize,
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            llm_api_url: env::var("ASTRA_LLM_URL")
                .unwrap_or_else(|_| "http://localhost:11434/api/generate".to_string()),
            llm_model: env::var("ASTRA_LLM_MODEL")
                .unwrap_or_else(|_| "gemma2:9b".to_string()),
            workspace_root: env::var("ASTRA_WORKSPACE")
                .map(PathBuf::from)
                .unwrap_or_else(|_| env::current_dir().unwrap_or_else(|_| PathBuf::from("."))),
            max_repair_attempts: env::var("ASTRA_MAX_ATTEMPTS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3),
        }
    }
}