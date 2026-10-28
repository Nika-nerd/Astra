mod agent;
mod config;
mod indexer;
mod llm;
mod server;
mod verifier;

use agent::AstraAgent;
use anyhow::Result;
use config::AppConfig;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    let cfg = AppConfig::from_env();
    info!("Astra Backend starting with config: {:?}", cfg);

    let agent = AstraAgent::new(cfg)?;
    server::run_stdio_server(agent).await?;

    Ok(())
}