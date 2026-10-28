use crate::llm::prompt::FilePatch;
use anyhow::{bail, Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub struct PatchTransaction {
    workspace_root: PathBuf,
    backups: HashMap<PathBuf, Option<String>>,
}

impl PatchTransaction {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            workspace_root,
            backups: HashMap::new(),
        }
    }

    pub fn apply(&mut self, patches: &[FilePatch]) -> Result<()> {
        for patch in patches {
            // Защита от выхода за пределы workspace (например, `../../etc`)
            if patch
                .relative_path
                .components()
                .any(|c| matches!(c, Component::ParentDir | Component::RootDir))
            {
                bail!("Unsafe file path in patch: {:?}", patch.relative_path);
            }

            let full_path = self.workspace_root.join(&patch.relative_path);

            // Сохраняем оригинал только при первом изменении файла в рамках транзакции
            if !self.backups.contains_key(&full_path) {
                let original = if full_path.exists() {
                    Some(
                        fs::read_to_string(&full_path)
                            .with_context(|| format!("Failed to backup {:?}", full_path))?,
                    )
                } else {
                    None
                };
                self.backups.insert(full_path.clone(), original);
            }

            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("Failed to create directory {:?}", parent))?;
            }

            fs::write(&full_path, &patch.content)
                .with_context(|| format!("Failed to write patch to {:?}", full_path))?;
        }

        Ok(())
    }

    pub fn rollback(&mut self) -> Result<()> {
        for (path, original) in self.backups.drain() {
            match original {
                Some(content) => {
                    fs::write(&path, content)
                        .with_context(|| format!("Failed to restore {:?}", path))?;
                }
                None => {
                    if path.exists() {
                        fs::remove_file(&path)
                            .with_context(|| format!("Failed to remove newly created {:?}", path))?;
                    }
                }
            }
        }
        Ok(())
    }
}

pub fn read_target_files(workspace_root: &Path, relative_paths: &[String]) -> Vec<(String, String)> {
    let mut result = Vec::new();
    for rel in relative_paths {
        let full = workspace_root.join(rel);
        if let Ok(content) = fs::read_to_string(full) {
            result.push((rel.clone(), content));
        }
    }
    result
}