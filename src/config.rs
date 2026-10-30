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
                .unwrap_or_else(|_| "gemma4".to_string()),
            workspace_root: env::var("ASTRA_WORKSPACE")
                .map(PathBuf::from)
                .unwrap_or_else(|_| env::current_dir().unwrap_or_else(|_| PathBuf::from("."))),
            max_repair_attempts: env::var("ASTRA_MAX_ATTEMPTS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
        }
    }

    #[allow(dead_code)]
    pub fn is_local(&self) -> bool {
        self.llm_api_url.contains("localhost") || self.llm_api_url.contains("127.0.0.1")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_is_local_localhost() {
        let config = AppConfig {
            llm_api_url: "http://localhost:11434".to_string(),
            llm_model: "test".to_string(),
            workspace_root: PathBuf::new(),
            max_repair_attempts: 1,
        };
        assert!(config.is_local());
    }

    #[test]
    fn test_is_local_ip() {
        let config = AppConfig {
            llm_api_url: "http://127.0.0.1:8080".to_string(),
            llm_model: "test".to_string(),
            workspace_root: PathBuf::new(),
            max_repair_attempts: 1,
        };
        assert!(config.is_local());
    }

    #[test]
    fn test_is_local_false() {
        let config = AppConfig {
            llm_api_url: "http://external.api.com".to_string(),
            llm_model: "test".to_string(),
            workspace_root: PathBuf::new(),
            max_repair_attempts: 1,
        };
        assert!(!config.is_local());
    }
}
