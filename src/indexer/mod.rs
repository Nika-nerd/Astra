pub mod ast;
pub mod scanner;

use anyhow::Result;
use std::fs;
use std::path::Path;
use tracing::warn;

#[derive(Debug, Clone)]
pub struct RepoMap {
    pub rendered: String,
}

impl RepoMap {
    pub fn build(workspace_root: &Path) -> Result<Self> {
        let files = scanner::scan_workspace(workspace_root)?;
        let mut out = String::new();

        out.push_str("=== PROJECT MANIFESTS ===\n");
        for toml_path in &files.cargo_tomls {
            let rel = toml_path.strip_prefix(workspace_root).unwrap_or(toml_path);
            if let Ok(content) = fs::read_to_string(toml_path) {
                out.push_str(&format!("--- {} ---\n{}\n\n", rel.display(), content.trim()));
            }
        }

        out.push_str("=== PROJECT AST SKELETON ===\n");
        for rs_path in &files.rust_files {
            let rel = rs_path.strip_prefix(workspace_root).unwrap_or(rs_path);
            match ast::extract_skeleton(rs_path) {
                Ok(skeleton) => {
                    out.push_str(&format!("--- {} ---\n", rel.display()));
                    for item in skeleton.items {
                        out.push_str(&item);
                        out.push('\n');
                    }
                    out.push('\n');
                }
                Err(e) => {
                    warn!("Skipping AST for {:?}: {}", rel, e);
                }
            }
        }

        Ok(Self { rendered: out })
    }
}
