use crate::agent::{AgentResult, AgentTask, AstraAgent};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{error, info};

#[derive(Deserialize)]
pub struct RpcRequest {
    pub id: String,
    pub task: AgentTask,
}

#[derive(Serialize)]
pub struct RpcResponse {
    pub id: String,
    pub result: Option<AgentResult>,
    pub error: Option<String>,
}

pub async fn run_stdio_server(agent: AstraAgent) -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut reader = BufReader::new(stdin).lines();

    info!("Astra stdio JSON-RPC server listening for VS Code requests...");

    while let Some(line) = reader.next_line().await? {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<RpcRequest>(trimmed) {
            Ok(req) => {
                info!("Received task [id={}]", req.id);
                match agent.execute(req.task).await {
                    Ok(res) => RpcResponse {
                        id: req.id,
                        result: Some(res),
                        error: None,
                    },
                    Err(e) => {
                        error!("Task execution error: {:#}", e);
                        RpcResponse {
                            id: req.id,
                            result: None,
                            error: Some(format!("{:#}", e)),
                        }
                    }
                }
            }
            Err(e) => RpcResponse {
                id: "unknown".to_string(),
                result: None,
                error: Some(format!("Invalid JSON request: {}", e)),
            },
        };

        let mut payload = serde_json::to_string(&response)?;
        payload.push('\n');
        stdout.write_all(payload.as_bytes()).await?;
        stdout.flush().await?;
    }

    Ok(())
}