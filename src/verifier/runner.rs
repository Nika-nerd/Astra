use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;
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
    let main_rs = workspace_root.join("src").join("main.rs");
    if main_rs.exists() {
        if let Ok(content) = std::fs::read_to_string(&main_rs) {
            if content.contains("println!(\"Hello, world!\");") && content.lines().count() <= 5 {
                warn!("src/main.rs still has default Hello World stub! Failing verification so LLM writes real code.");
                return Ok(VerificationResult {
                    success: false,
                    error_log: "INCOMPLETE IMPLEMENTATION ERROR: `src/main.rs` still contains the default `println!(\"Hello, world!\");` stub from `cargo init`! You only updated Cargo.toml or forgot to output `=== FILE: src/main.rs ===`. You MUST output the complete implementation for `src/main.rs` and all planned modules now.".to_string(),
                });
            }
        }
    }
    info!("Running cargo clippy in {:?}...", workspace_root);
    let clippy_res = run_clippy(workspace_root).await?;
    if !clippy_res.success {
        return Ok(clippy_res);
    }

    info!("Running cargo test (with 15s hang protection) in {:?}...", workspace_root);
    let test_res = run_tests(workspace_root).await?;
    if !test_res.success {
        return Ok(test_res);
    }

    // Форматируем финальный проверенный код за миллисекунды
    let _ = Command::new("cargo")
        .args(["fmt"])
        .current_dir(workspace_root)
        .output()
        .await;

    Ok(test_res)
}

async fn run_clippy(workspace_root: &Path) -> Result<VerificationResult> {
    // 1. Автоматически применяем безопасные фиксы компилятора
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

    // 2. Превращаем критические желтые предупреждения (бесконечные циклы, дедлоки, todo!) в ошибки (-D),
    // но игнорируем косметические предупреждения (-A), чтобы не терять скорость
    let output = Command::new("cargo")
        .args([
            "clippy",
            "--all-targets",
            "--message-format=json",
            "--",
            "-D", "clippy::infinite_iter",
            "-D", "while_true",
            "-D", "clippy::never_loop",
            "-D", "clippy::await_holding_lock",
            "-D", "clippy::todo",
            "-D", "clippy::unimplemented",
            "-D", "unreachable_code",
            "-A", "dead_code",
            "-A", "unused_imports",
            "-A", "unused_variables",
        ])
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
    // Сначала быстро собираем тестовый бинарник без запуска, чтобы время компиляции не съедало таймаут рантайма
    let build_out = Command::new("cargo")
        .args(["test", "--no-run", "--quiet"])
        .current_dir(workspace_root)
        .output()
        .await
        .context("Failed to build tests")?;

    if !build_out.status.success() {
        return Ok(VerificationResult {
            success: false,
            error_log: String::from_utf8_lossy(&build_out.stderr).to_string(),
        });
    }

    // Запускаем сами тесты с жестким таймаутом 15 секунд для перехвата бесконечных циклов
    let child = Command::new("cargo")
        .args(["test", "--color=never", "--", "--test-threads=4"])
        .current_dir(workspace_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .context("Failed to spawn cargo test")?;

    match timeout(Duration::from_secs(15), child.wait_with_output()).await {
        Ok(Ok(output)) => {
            if output.status.success() {
                Ok(VerificationResult {
                    success: true,
                    error_log: String::new(),
                })
            } else {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                Ok(VerificationResult {
                    success: false,
                    error_log: format!("TEST FAILURE:\n{}\n{}", stdout, stderr),
                })
            }
        }
        Ok(Err(e)) => Err(e).context("Failed waiting for cargo test"),
        Err(_) => {
            warn!("cargo test timed out after 15 seconds! Infinite loop detected.");
            Ok(VerificationResult {
                success: false,
                error_log: "CRITICAL RUNTIME ERROR: `cargo test` timed out after 15 seconds! Your code or test contains an infinite loop (`loop {}` or `while` condition that never terminates) or a blocking network listener inside a unit test. Fix the loop termination condition immediately.".to_string(),
            })
        }
    }}