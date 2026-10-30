use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;
use tokio::process::Command;
use tracing::{debug, info, warn};

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

pub async fn ensure_cargo_project(workspace_root: &Path) -> Result<()> {
    let cargo_toml = workspace_root.join("Cargo.toml");
    if !cargo_toml.exists() {
        info!("No Cargo.toml found in {:?}. Running `cargo init`...", workspace_root);
        let output = Command::new("cargo")
            .args(["init", "--bin"])
            .current_dir(workspace_root)
            .output()
            .await
            .context("Failed to run cargo init")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("cargo init warning/error: {}", stderr);
        }
    }
    Ok(())
}

pub async fn run_workspace_commands(workspace_root: &Path, commands: &[String]) -> Result<Vec<String>> {
    let mut executed = Vec::new();

    for cmd_line in commands {
        let parts: Vec<&str> = cmd_line.split_whitespace().collect();
        let Some(&program) = parts.first() else { continue };

        // Разрешаем только безопасные команды сборки и структуры проекта
        if program != "cargo" && program != "mkdir" {
            warn!("Skipping disallowed terminal command: {}", cmd_line);
            continue;
        }

        // Пропускаем ручные запуски test/clippy/run из блока команд, так как верификатор сам их запускает
        if program == "cargo" && parts.get(1).is_some_and(|&sub| sub == "test" || sub == "clippy" || sub == "run") {
            continue;
        }

        info!("Executing command in {:?}: {}", workspace_root, cmd_line);
        let status = Command::new(program)
            .args(&parts[1..])
            .current_dir(workspace_root)
            .output()
            .await;

        match status {
            Ok(out) if out.status.success() => {
                executed.push(cmd_line.clone());
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                warn!("Command `{}` failed: {}", cmd_line, stderr);
            }
            Err(e) => {
                warn!("Failed to spawn command `{}`: {}", cmd_line, e);
            }
        }
    }

    Ok(executed)
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
    // 1. Автоматически применяем безопасные фиксы компилятора (unused imports, mut и т.д.)
    let _ = Command::new("cargo")
        .args([
            "clippy",
            "--fix",
            "--allow-dirty",
            "--allow-staged",
            "--all-targets",
            "--quiet",
        ])
        .current_dir(workspace_root)
        .output()
        .await;

    // 2. Запускаем проверку без -D warnings, чтобы не валить сборку из-за мелких советов стиля
    let output = Command::new("cargo")
        .args(["clippy", "--all-targets", "--message-format=json"])
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
                    if diag.level == "error" {
                        if let Some(rendered) = diag.rendered {
                            diagnostics.push(rendered);
                        }
                    }
                }
            }
        }
    }

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

    // Если ошибка произошла на уровне самого Cargo (например, битый Cargo.toml), берем stderr
   
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