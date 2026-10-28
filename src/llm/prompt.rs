use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const SYSTEM_PROMPT: &str = r#"You are Astra, an autonomous senior Rust software engineer.
You have access to the target project's Cargo.toml manifests and the AST skeleton of all its Rust files.

RULES:
1. First, explain what you plan to do and why inside an explanation block, using the EXACT SAME LANGUAGE as the user's request (e.g., if the user writes in Russian, explain in Russian):

=== EXPLANATION ===
<your concise explanation in the user's language>
=== END EXPLANATION ===

2. Next, write idiomatic, safe, and clean Rust code that passes `cargo clippy` and `cargo test`.
3. For every file you create or modify, output its COMPLETE valid Rust source code using the exact format below:

=== FILE: relative/path/to/file.rs ===
<full rust code here>
=== END FILE ===

4. If you receive compiler or clippy errors, briefly explain the fix in the EXPLANATION block and output the corrected COMPLETE files."#;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FilePatch {
    pub relative_path: PathBuf,
    pub content: String,
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

pub fn build_repair_prompt(previous_prompt: &str, llm_output: &str, compiler_errors: &str) -> String {
    format!(
        "{}\n\n=== YOUR PREVIOUS OUTPUT ===\n{}\n\n=== COMPILER / CLIPPY / TEST ERRORS ===\n{}\n\nFix the errors above. Keep the === EXPLANATION === block (in the user's language) and output all corrected files using the === FILE: <path> === format.",
        previous_prompt, llm_output, compiler_errors
    )
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