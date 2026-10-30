use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const PLAN_SYSTEM_PROMPT: &str = r#"You are Agent Astra, an autonomous senior Rust software architect.
You have visibility into the target workspace's AST and Cargo manifests (if any exist).

Your task in this phase is ONLY to create or refine a clear, step-by-step implementation plan before writing code.
1. Respond in the EXACT SAME LANGUAGE as the user's request.
2. Structure the plan clearly:
   - Goal & Architecture overview
   - Terminal / Cargo commands to run (e.g. `cargo add ...`)
   - Files to create or modify (with brief description of structs/functions)
   - Verification strategy (`cargo clippy` & `cargo test`)
3. Do NOT output full source code files yet. Wait for the user's approval."#;

pub const SYSTEM_PROMPT: &str = r#"You are Astra, an autonomous senior Rust software engineer.
You are now executing an APPROVED architectural plan to build or update a Rust project.

RULES:
1. First, briefly summarize what you are building in the user's language:
=== EXPLANATION ===
<summary in user's language>
=== END EXPLANATION ===

2. If any Cargo/shell commands are needed BEFORE compiling (e.g. `cargo add serde --features derive`), list them (one per line) inside:
=== COMMANDS ===
<command 1>
=== END COMMANDS ===
(Only `cargo` and `mkdir` commands are allowed. Do not run `cargo test` or `cargo clippy` here, Astra runs them automatically).

3. Output COMPLETE valid Rust source code (and Cargo.toml if creating/updating dependencies) for EVERY file in the plan:
=== FILE: relative/path/to/file.rs ===
<full code here>
=== END FILE ===

4. When fixing compiler/clippy/test errors on repair attempts, you ONLY need to output the === FILE: <path> === blocks for the specific files that need changes (each modified file must still have its complete code)."#;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FilePatch {
    pub relative_path: PathBuf,
    pub content: String,
}

pub fn build_plan_prompt(
    repo_map: &str,
    active_files: &[(String, String)],
    user_task: &str,
    previous_plan: Option<&str>,
) -> String {
    let mut prompt = String::new();
    prompt.push_str(&format!("{}\n", repo_map));

    if !active_files.is_empty() {
        prompt.push_str("=== REFERENCED FILES ===\n");
        for (path, content) in active_files {
            prompt.push_str(&format!("--- {} ---\n{}\n\n", path, content));
        }
    }

    if let Some(prev) = previous_plan {
        prompt.push_str("=== PREVIOUS PLAN ===\n");
        prompt.push_str(prev);
        prompt.push_str("\n\n=== USER FEEDBACK / REQUESTED CHANGES TO PLAN ===\n");
        prompt.push_str(user_task);
        prompt.push_str("\n\nUpdate and rewrite the complete architectural plan according to the user's feedback.");
    } else {
        prompt.push_str("=== USER PROJECT REQUEST ===\n");
        prompt.push_str(user_task);
        prompt.push_str("\n\nCreate a detailed step-by-step implementation plan for approval.");
    }

    prompt
}

pub fn build_initial_prompt(
    repo_map: &str,
    active_files: &[(String, String)],
    user_task: &str,
) -> String {
    let mut prompt = String::new();
    prompt.push_str(&format!("{}\n", repo_map));

    if !active_files.is_empty() {
        prompt.push_str("=== TARGET FILES FULL CONTENT ===\n");
        for (path, content) in active_files {
            prompt.push_str(&format!("--- {} ---\n{}\n\n", path, content));
        }
    }

    prompt.push_str("=== USER TASK ===\n");
    prompt.push_str(user_task);
    prompt.push_str("\n\nProvide the updated or new files using the === FILE: <path> === format.");
    prompt
}

pub fn build_repair_prompt(
    task_and_plan: &str,
    current_repo_map: &str,
    current_files: &[(String, String)],
    compiler_errors: &str,
) -> String {
    let mut prompt = String::new();
    prompt.push_str("=== TASK & APPROVED PLAN ===\n");
    prompt.push_str(task_and_plan);
    prompt.push_str("\n\n");
    prompt.push_str(current_repo_map);
    prompt.push_str("\n=== CURRENT PROJECT FILES ON DISK ===\n");
    for (path, content) in current_files {
        prompt.push_str(&format!("--- {} ---\n{}\n\n", path, content));
    }
    prompt.push_str("=== COMPILER / CLIPPY / TEST ERRORS TO FIX ===\n");
    prompt.push_str(compiler_errors);
    prompt.push_str(
        "\n\nAnalyze the errors above. Output ONLY the files that need to be fixed or created using the === FILE: <path> === ... === END FILE === format (include any missing Cargo commands in === COMMANDS === if needed).",
    );
    prompt
}
pub fn parse_explanation(llm_response: &str) -> String {
    let start_marker = concat!("=== ", "EXPLANATION ===");
    let end_marker = concat!("=== ", "END EXPLANATION ===");

    if let Some(start_idx) = llm_response.find(start_marker) {
        let content_start = start_idx + start_marker.len();
        let rest = &llm_response[content_start..];
        if let Some(end_idx) = rest.find(end_marker) {
            return rest[..end_idx].trim().to_string();
        }
    }

    // Fallback: если модель написала текст до первого === FILE:, берем его как объяснение
    let file_marker = concat!("=== ", "FILE:");
    if let Some(file_idx) = llm_response.find(file_marker) {
        let before = llm_response[..file_idx].trim();
        if !before.is_empty() {
            return before.to_string();
        }
    }

    "Changes have been prepared and verified.".to_string()
}

pub fn parse_commands(llm_response: &str) -> Vec<String> {
    let start_marker = concat!("=== ", "COMMANDS ===");
    let end_marker = concat!("=== ", "END COMMANDS ===");

    if let Some(start_idx) = llm_response.find(start_marker) {
        let content_start = start_idx + start_marker.len();
        let rest = &llm_response[content_start..];
        if let Some(end_idx) = rest.find(end_marker) {
            return rest[..end_idx]
                .lines()
                .map(|l| l.trim().trim_start_matches('$').trim().to_string())
                .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with("```"))
                .collect();
        }
    }
    Vec::new()
}


pub fn parse_file_patches(llm_response: &str) -> Vec<FilePatch> {
    let mut patches = Vec::new();
    let start_marker = concat!("=== ", "FILE:");
    let end_marker = concat!("\n=== ", "END FILE ===");

    let mut current_pos = 0;
    while let Some(start_idx) = llm_response[current_pos..].find(start_marker) {
        let abs_start = current_pos + start_idx + start_marker.len();
        let rest = &llm_response[abs_start..];

        let Some(line_end) = rest.find('\n') else { break };
        let header_line = rest[..line_end].trim();
        let path_str = header_line.trim_end_matches("===").trim();

        let content_start = abs_start + line_end + 1;
        let search_area = &llm_response[content_start..];

        // Если внутри файла встречается еще один === FILE:, ищем закрывающий маркер до него, иначе берем последний
        let next_file_idx = search_area.find(start_marker).unwrap_or(search_area.len());
        let Some(end_idx) = search_area[..next_file_idx].rfind(end_marker) else { break };

        let mut raw_content = search_area[..end_idx].trim();

        if raw_content.starts_with("```") {
            if let Some(first_nl) = raw_content.find('\n') {
                raw_content = &raw_content[first_nl + 1..];
            }
            if raw_content.ends_with("```") {
                raw_content = raw_content[..raw_content.len() - 3].trim_end();
            }
        }

        if !path_str.is_empty() {
            patches.push(FilePatch {
                relative_path: PathBuf::from(path_str),
                content: format!("{}\n", raw_content),
            });
        }

        current_pos = content_start + end_idx + end_marker.len();
    }

    patches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_file_patches() {
        let sample = r#"
=== FILE: src/math.rs ===
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
=== END FILE === "#; let patches = parse_file_patches(sample); 
assert_eq!(patches.len(), 1); assert_eq!(patches[0].relative_path, PathBuf::from("src/math.rs"));
 assert!(patches[0].content.contains("pub fn add"));
 assert!(!patches[0].content.contains("```")); } }