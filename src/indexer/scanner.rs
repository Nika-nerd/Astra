use anyhow::Result;
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};
use tracing::debug;

#[derive(Debug, Clone)]
pub struct ProjectFiles {
    pub rust_files: Vec<PathBuf>,
    pub cargo_tomls: Vec<PathBuf>,
}

pub fn scan_workspace(root: &Path) -> Result<ProjectFiles> {
    let mut rust_files = Vec::new();
    let mut cargo_tomls = Vec::new();

    let walker = WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .build();

      for result in walker {
        let entry = match result {
            Ok(e) => e,
            Err(_) => continue,
        };

        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }

        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            debug!("Found Rust file: {:?}", path);
            rust_files.push(path.to_path_buf());
        } else if path.file_name().and_then(|s| s.to_str()) == Some("Cargo.toml") {
            debug!("Found Cargo.toml: {:?}", path);
            cargo_tomls.push(path.to_path_buf());
        }
    }

    rust_files.sort();
    cargo_tomls.sort();

    Ok(ProjectFiles {
        rust_files,
        cargo_tomls,
    })
}