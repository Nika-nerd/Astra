use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;
use tokio::process::Command;
use tracing::{debug, info};

#[derive(Debug, Clone)]
pub struct VerificationResult {
    pub success: bool,
    pub error_log: String,
}

#[derive(Deserialize)]
struct CargoMessage {
    reason: String,
    message: Option<CompilerDiagnostic>,
}

#[derive(Deserialize)]
struct CompilerDiagnostic {
    level: String,
    rendered: Option<String>,
}

pub async fn run_verification(workspace_root: &Path) -> Result<VerificationResult> {
    info!("Running cargo clippy in {:?}...", workspace_root);
    let clippy_res = run_clippy(workspace_root).await?;
    if !clippy_res.success {
        return Ok(clippy_res);
    }

    info!("Running cargo test in {:?}...", workspace_root);
    run_tests(workspace_root).await
}

async fn run_clippy(workspace_root: &Path) -> Result<VerificationResult> {
    let output = Command::new("cargo")
        .args(["clippy", "--all-targets", "--message-format=json", "--", "-D", "warnings"])
        .current_dir(workspace_root)
        .output()
        .await
        .context("Failed to execute cargo clippy")?;

    if output.status.success() {
        return Ok(VerificationResult {
            success: true,
            error_log: String::new(),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut diagnostics = Vec::new();

    for line in stdout.lines() {
        if let Ok(msg) = serde_json::from_str::<CargoMessage>(line) {
            if msg.reason == "compiler-message" {
                if let Some(diag) = msg.message {
                    if diag.level == "error" || diag.level == "warning" {
                        if let Some(rendered) = diag.rendered {
                            diagnostics.push(rendered);
                        }
                    }
                }
            }
        }
    }

    // Если ошибка произошла на уровне самого Cargo (например, битый Cargo.toml), берем stderr
    if diagnostics.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        diagnostics.push(stderr.to_string());
    }

    let combined = diagnostics.join("\n");
    debug!("Clippy failed with diagnostics:\n{}", combined);

    Ok(VerificationResult {
        success: false,
        error_log: combined,
    })
}

async fn run_tests(workspace_root: &Path) -> Result<VerificationResult> {
    let output = Command::new("cargo")
        .args(["test", "--color=never"])
        .current_dir(workspace_root)
        .output()
        .await
        .context("Failed to execute cargo test")?;

    if output.status.success() {
        return Ok(VerificationResult {
            success: true,
            error_log: String::new(),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    Ok(VerificationResult {
        success: false,
        error_log: format!("STDOUT:\n{}\nSTDERR:\n{}", stdout, stderr),
    })
}