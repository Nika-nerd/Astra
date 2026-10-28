#![allow(dead_code)]

mod config;
mod indexer;
mod llm;
mod verifier;

use anyhow::Result;
use config::AppConfig;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let cfg = AppConfig::from_env();
    info!("Astra Backend starting with config: {:?}", cfg);

    let repo_map = indexer::RepoMap::build(&cfg.workspace_root)?;
    info!("Generated Repo Map ({} bytes)", repo_map.rendered.len());

    let _llm_client = llm::client::LlmClient::new(cfg.llm_api_url.clone(), cfg.llm_model.clone())?;
    info!("LLM client initialized for model: {}", cfg.llm_model);
    
    Ok(())
}