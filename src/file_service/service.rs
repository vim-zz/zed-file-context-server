use crate::editor::file_editor::FileEditor;
use crate::file_service::backup::BackupManager;
use crate::shared::logging;
use std::path::{Path, PathBuf};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use thiserror::Error;
use serde_json::json;

#[derive(Error, Debug)]
pub enum FileServiceError {
    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Line number out of range: {0}")]
    LineNumberOutOfRange(usize),

    #[error("File already exists: {0}")]
    FileAlreadyExists(String),

    #[error("Backup error: {0}")]
    BackupError(#[from] crate::file_service::backup::BackupError),
}

pub struct FileService {
    base_directory: PathBuf,
    editor: FileEditor,
    backup_manager: BackupManager,
}

impl FileService {
    pub fn new(base_directory: &PathBuf) -> Result<Self, FileServiceError> {
        if !base_directory.exists() {
            return Err(FileServiceError::InvalidPath(format!(
                "Base directory does not exist: {}",
                base_directory.display()
            )));
        }

        let editor = FileEditor::new();
        let backup_manager = BackupManager::new(base_directory)?;

        Ok(Self {
            base_directory: base_directory.clone(),
            editor,
            backup_manager,
        })
    }

    pub fn change_directory(&mut self, new_directory: &PathBuf) -> Result<(), FileServiceError> {
        if !new_directory.exists() {
            return Err(FileServiceError::InvalidPath(format!(
                "Directory does not exist: {}",
                new_directory.display()
            )));
        }

        self.base_directory = new_directory.clone();
        self.backup_manager = BackupManager::new(new_directory)?;

        logging::info(&format!(
            "File service directory changed to: {}",
            new_directory.display()
        ));

        Ok(())
    }

    // Resolves a path relative to the base directory
    // This prevents accessing files outside the base directory for safety
    fn resolve_path(&self, path: &Path) -> Result<PathBuf, FileServiceError> {
        let mut resolved_path = self.base_directory.clone();

        // If path is absolute, verify it's within the base directory
        if path.is_absolute() {
            let path_str = path.to_string_lossy().to_string();
            let base_str = self.base_directory.to_string_lossy().to_string();

            if !path_str.starts_with(&base_str) {
                return Err(FileServiceError::PermissionDenied(format!(
                    "Cannot access files outside the project directory: {}",
                    path.display()
                )));
            }

            resolved_path = path.to_path_buf();
        } else {
            // For relative paths, simply join with base directory
            resolved_path.push(path);
        }

        // Canonicalize to resolve any .. or symlinks, then verify still in base directory
        match resolved_path.canonicalize() {
            Ok(canon_path) => {
                let canon_base = self.base_directory.canonicalize()?;
                if !canon_path.starts_with(&canon_base) {
                    return Err(FileServiceError::PermissionDenied(format!(
                        "Path escapes the project directory: {}",
                        path.display()
                    )));
                }
                Ok(canon_path)
            }
            Err(e) => {
                // If canonicalization fails (e.g., file doesn't exist), just return the joined path
                // This is needed for operations like creating a new file
                Ok(resolved_path)
            }
        }
    }

    // File Reading Operations

    pub async fn read_file(&self, path: &Path) -> anyhow::Result<String> {
        let resolved_path = self.resolve_path(path)?;

        if !resolved_path.exists() {
            return Err(FileServiceError::FileNotFound(
                resolved_path.to_string_lossy().to_string(),
            ).into());
        }

        self.editor.read_file(&resolved_path).await.map_err(|e| e.into())
    }

    pub async fn file_exists(&self, path: &Path) -> bool {
        match self.resolve_path(path) {
            Ok(resolved) => resolved.exists(),
            Err(_) => false,
        }
    }

    pub async fn is_file(&self, path: &Path) -> anyhow::Result<bool> {
        let resolved_path = self.resolve_path(path)?;
        Ok(resolved_path.is_file())
    }

    pub async fn is_directory(&self, path: &Path) -> anyhow::Result<bool> {
        let resolved_path = self.resolve_path(path)?;
        Ok(resolved_path.is_dir())
    }

    // File Writing Operations

    pub async fn write_file(&self, path: &Path, content: &str) -> anyhow::Result<()> {
        let resolved_path = self.resolve_path(path)?;

        // Create a backup before modifying
        if resolved_path.exists() {
            self.backup_manager.create_backup(&resolved_path).await?;
        }

        self.editor.write_file(&resolved_path, content).await.map_err(|e| e.into())
    }

    pub async fn append_to_file(&self, path: &Path, content: &str) -> anyhow::Result<()> {
        let resolved_path = self.resolve_path(path)?;

        if !resolved_path.exists() {
            return Err(FileServiceError::FileNotFound(
                resolved_path.to_string_lossy().to_string(),
            ).into());
        }

        // Create a backup before modifying
        self.backup_manager.create_backup(&resolved_path).await?;

        self.editor.append_to_file(&resolved_path, content).await.map_err(|e| e.into())
    }

    pub async fn create_file(&self, path: &Path, content: &str) -> anyhow::Result<()> {
        let resolved_path = self.resolve_path(path)?;

        if resolved_path.exists() {
            return Err(FileServiceError::FileAlreadyExists(
                resolved_path.to_string_lossy().to_string(),
            ).into());
        }

        // Ensure parent directory exists
        if let Some(parent) = resolved_path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }

        self.editor.write_file(&resolved_path, content).await.map_err(|e| e.into())
    }

    // Line-based editing operations

    pub async fn insert_line(&self, path: &Path, line_num: usize, content: &str) -> anyhow::Result<()> {
        let resolved_path = self.resolve_path(path)?;

        if !resolved_path.exists() {
            return Err(FileServiceError::FileNotFound(
                resolved_path.to_string_lossy().to_string(),
            ).into());
        }

        // Create a backup before modifying
        self.backup_manager.create_backup(&resolved_path).await?;

        self.editor.insert_line(&resolved_path, line_num, content).await.map_err(|e| e.into())
    }

    pub async fn replace_line(&self, path: &Path, line_num: usize, content: &str) -> anyhow::Result<()> {
        let resolved_path = self.resolve_path(path)?;

        if !resolved_path.exists() {
            return Err(FileServiceError::FileNotFound(
                resolved_path.to_string_lossy().to_string(),
            ).into());
        }

        // Create a backup before modifying
        self.backup_manager.create_backup(&resolved_path).await?;

        self.editor.replace_line(&resolved_path, line_num, content).await.map_err(|e| e.into())
    }

    pub async fn delete_line(&self, path: &Path, line_num: usize) -> anyhow::Result<()> {
        let resolved_path = self.resolve_path(path)?;

        if !resolved_path.exists() {
            return Err(FileServiceError::FileNotFound(
                resolved_path.to_string_lossy().to_string(),
            ).into());
        }

        // Create a backup before modifying
        self.backup_manager.create_backup(&resolved_path).await?;

        self.editor.delete_line(&resolved_path, line_num).await.map_err(|e| e.into())
    }

    pub async fn edit_region(
        &self,
        path: &Path,
        start_line: usize,
        end_line: usize,
        new_content: &str,
    ) -> anyhow::Result<()> {
        let resolved_path = self.resolve_path(path)?;

        if !resolved_path.exists() {
            return Err(FileServiceError::FileNotFound(
                resolved_path.to_string_lossy().to_string(),
            ).into());
        }

        // Create a backup before modifying
        self.backup_manager.create_backup(&resolved_path).await?;

        self.editor.edit_region(&resolved_path, start_line, end_line, new_content)
            .await
            .map_err(|e| e.into())
    }

    // File management operations

    pub async fn delete_file(&self, path: &Path) -> anyhow::Result<()> {
        let resolved_path = self.resolve_path(path)?;

        if !resolved_path.exists() {
            return Err(FileServiceError::FileNotFound(
                resolved_path.to_string_lossy().to_string(),
            ).into());
        }

        // Create a backup before deleting
        self.backup_manager.create_backup(&resolved_path).await?;

        if resolved_path.is_file() {
            std::fs::remove_file(&resolved_path)?;
            logging::info(&format!("Deleted file: {}", resolved_path.display()));
            Ok(())
        } else {
            Err(anyhow::anyhow!("Not a file: {}", resolved_path.display()))
        }
    }

    pub async fn rename_file(&self, from_path: &Path, to_path: &Path) -> anyhow::Result<()> {
        let resolved_from = self.resolve_path(from_path)?;
        let resolved_to = self.resolve_path(to_path)?;

        if !resolved_from.exists() {
            return Err(FileServiceError::FileNotFound(
                resolved_from.to_string_lossy().to_string(),
            ).into());
        }

        if resolved_to.exists() {
            return Err(FileServiceError::FileAlreadyExists(
                resolved_to.to_string_lossy().to_string(),
            ).into());
        }

        // Create a backup before renaming
        self.backup_manager.create_backup(&resolved_from).await?;

        // Ensure parent directory of target exists
        if let Some(parent) = resolved_to.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }

        std::fs::rename(&resolved_from, &resolved_to)?;
        logging::info(&format!(
            "Renamed file from {} to {}",
            resolved_from.display(),
            resolved_to.display()
        ));

        Ok(())
    }

    // Backup and restore operations

    pub async fn restore_backup(&self, path: &Path) -> anyhow::Result<()> {
        let resolved_path = self.resolve_path(path)?;
        self.backup_manager.restore_latest_backup(&resolved_path).await.map_err(|e| e.into())
    }

    pub async fn list_backups(&self, path: &Path) -> anyhow::Result<Vec<PathBuf>> {
        let resolved_path = self.resolve_path(path)?;
        self.backup_manager.list_backups(&resolved_path).await.map_err(|e| e.into())
    }

    // Suggestion handling

    pub async fn apply_suggestion(
        &self,
        path: &Path,
        suggestion: &serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        // Parse the suggestion to determine what kind of edit is needed
        // Format should be something like:
        // { "type": "edit", "edits": [...] } or
        // { "type": "replace", "content": "..." }

        let edit_type = suggestion
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let resolved_path = self.resolve_path(path)?;

        // Create backup before proceeding
        if resolved_path.exists() {
            self.backup_manager.create_backup(&resolved_path).await?;
        }

        match edit_type {
            "replace" => {
                // Full file replacement
                if let Some(content) = suggestion.get("content").and_then(|v| v.as_str()) {
                    self.editor.write_file(&resolved_path, content).await?;
                    Ok(json!({
                        "success": true,
                        "action": "replace",
                        "path": resolved_path.to_string_lossy()
                    }))
                } else {
                    Err(anyhow::anyhow!("Missing 'content' field in replace suggestion"))
                }
            },
            "edit" => {
                // Line-by-line edits
                if let Some(edits) = suggestion.get("edits").and_then(|v| v.as_array()) {
                    // Apply each edit in sequence
                    let mut results = Vec::new();

                    for edit in edits {
                        let action = edit.get("action").and_then(|v| v.as_str()).unwrap_or("unknown");

                        match action {
                            "insert" => {
                                let line = edit.get("line").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                                let content = edit.get("content").and_then(|v| v.as_str()).unwrap_or("");
                                self.editor.insert_line(&resolved_path, line, content).await?;
                                results.push(json!({
                                    "action": "insert",
                                    "line": line,
                                    "status": "success"
                                }));
                            },
                            "replace" => {
                                let line = edit.get("line").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                                let content = edit.get("content").and_then(|v| v.as_str()).unwrap_or("");
                                self.editor.replace_line(&resolved_path, line, content).await?;
                                results.push(json!({
                                    "action": "replace",
                                    "line": line,
                                    "status": "success"
                                }));
                            },
                            "delete" => {
                                let line = edit.get("line").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                                self.editor.delete_line(&resolved_path, line).await?;
                                results.push(json!({
                                    "action": "delete",
                                    "line": line,
                                    "status": "success"
                                }));
                            },
                            "region" => {
                                let start = edit.get("start").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                                let end = edit.get("end").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                                let content = edit.get("content").and_then(|v| v.as_str()).unwrap_or("");
                                self.editor.edit_region(&resolved_path, start, end, content).await?;
                                results.push(json!({
                                    "action": "region",
                                    "start": start,
                                    "end": end,
                                    "status": "success"
                                }));
                            },
                            _ => {
                                results.push(json!({
                                    "action": action,
                                    "status": "error",
                                    "message": "Unknown edit action"
                                }));
                            }
                        }
                    }

                    Ok(json!({
                        "success": true,
                        "action": "edit",
                        "path": resolved_path.to_string_lossy(),
                        "results": results
                    }))
                } else {
                    Err(anyhow::anyhow!("Missing or invalid 'edits' field in edit suggestion"))
                }
            },
            "create" => {
                // Create a new file
                if let Some(content) = suggestion.get("content").and_then(|v| v.as_str()) {
                    // Don't overwrite existing files unless explicitly allowed
                    if resolved_path.exists() {
                        let overwrite = suggestion
                            .get("overwrite")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);

                        if !overwrite {
                            return Err(anyhow::anyhow!("File already exists and overwrite not specified"));
                        }

                        // Backup if we're going to overwrite
                        self.backup_manager.create_backup(&resolved_path).await?;
                    }

                    // Ensure parent directories exist
                    if let Some(parent) = resolved_path.parent() {
                        if !parent.exists() {
                            std::fs::create_dir_all(parent)?;
                        }
                    }

                    self.editor.write_file(&resolved_path, content).await?;

                    Ok(json!({
                        "success": true,
                        "action": "create",
                        "path": resolved_path.to_string_lossy()
                    }))
                } else {
                    Err(anyhow::anyhow!("Missing 'content' field in create suggestion"))
                }
            },
            _ => Err(anyhow::anyhow!("Unknown suggestion type: {}", edit_type))
        }
    }
}
