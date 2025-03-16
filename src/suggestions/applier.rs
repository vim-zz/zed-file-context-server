use std::path::{Path, PathBuf};
use serde_json::{json, Value};
use crate::editor::file_editor::FileEditor;
use crate::shared::logging;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SuggestionApplyError {
    #[error("Invalid suggestion format: {0}")]
    InvalidFormat(String),

    #[error("Failed to apply suggestion: {0}")]
    ApplicationFailed(String),

    #[error("File error: {0}")]
    FileError(String),

    #[error(transparent)]
    IoError(#[from] std::io::Error),
}

pub struct SuggestionApplier {
    editor: FileEditor,
}

impl SuggestionApplier {
    pub fn new() -> Self {
        Self {
            editor: FileEditor::new(),
        }
    }

    // Apply a parsed suggestion to a specific file
    pub async fn apply_suggestion(
        &self,
        file_path: &Path,
        suggestion: &Value,
    ) -> anyhow::Result<Value> {
        logging::info(&format!(
            "Applying suggestion to file: {}",
            file_path.display()
        ));

        // Get suggestion type
        let suggestion_type = match suggestion.get("type").and_then(|t| t.as_str()) {
            Some(t) => t,
            None => {
                return Err(SuggestionApplyError::InvalidFormat(
                    "Missing 'type' field in suggestion".to_string()
                ).into());
            }
        };

        match suggestion_type {
            "replace" => self.apply_replace_suggestion(file_path, suggestion).await,
            "edit" => self.apply_edit_suggestion(file_path, suggestion).await,
            "create" => self.apply_create_suggestion(file_path, suggestion).await,
            _ => Err(SuggestionApplyError::InvalidFormat(
                format!("Unsupported suggestion type: {}", suggestion_type)
            ).into()),
        }
    }

    // Apply a full file replacement
    async fn apply_replace_suggestion(
        &self,
        file_path: &Path,
        suggestion: &Value,
    ) -> anyhow::Result<Value> {
        // Get content to replace with
        let content = match suggestion.get("content").and_then(|c| c.as_str()) {
            Some(c) => c,
            None => {
                return Err(SuggestionApplyError::InvalidFormat(
                    "Missing 'content' field in replace suggestion".to_string()
                ).into());
            }
        };

        // Make sure parent directory exists
        if let Some(parent) = file_path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }

        // Write the content to the file
        self.editor.write_file(file_path, content).await?;

        Ok(json!({
            "success": true,
            "action": "replace",
            "file": file_path.to_string_lossy()
        }))
    }

    // Apply multiple edits to a file
    async fn apply_edit_suggestion(
        &self,
        file_path: &Path,
        suggestion: &Value,
    ) -> anyhow::Result<Value> {
        // Get edits array
        let edits = match suggestion.get("edits").and_then(|e| e.as_array()) {
            Some(e) => e,
            None => {
                return Err(SuggestionApplyError::InvalidFormat(
                    "Missing 'edits' array in edit suggestion".to_string()
                ).into());
            }
        };

        if edits.is_empty() {
            return Ok(json!({
                "success": true,
                "action": "edit",
                "file": file_path.to_string_lossy(),
                "edits_applied": 0,
                "message": "No edits to apply"
            }));
        }

        // Make sure the file exists
        if !file_path.exists() {
            return Err(SuggestionApplyError::FileError(
                format!("File does not exist: {}", file_path.display())
            ).into());
        }

        // Apply each edit in sequence
        let mut results = Vec::new();
        let mut edits_applied = 0;

        // We need to read the file content first
        let original_content = self.editor.read_file(file_path).await?;
        let mut lines: Vec<&str> = original_content.lines().collect();

        for edit in edits {
            let action = edit.get("action").and_then(|a| a.as_str()).unwrap_or("unknown");

            match action {
                "insert" => {
                    if let (Some(line), Some(content)) = (
                        edit.get("line").and_then(|l| l.as_u64()),
                        edit.get("content").and_then(|c| c.as_str())
                    ) {
                        let line_num = line as usize;

                        // Line number might be 1-based, so handle both possibilities
                        if line_num <= lines.len() {
                            // Insert at the given position
                            lines.insert(line_num, content);
                            edits_applied += 1;

                            results.push(json!({
                                "action": "insert",
                                "line": line_num,
                                "status": "success"
                            }));
                        } else {
                            results.push(json!({
                                "action": "insert",
                                "line": line_num,
                                "status": "error",
                                "message": "Line number out of range"
                            }));
                        }
                    }
                },
                "replace" => {
                    if let (Some(line), Some(content)) = (
                        edit.get("line").and_then(|l| l.as_u64()),
                        edit.get("content").and_then(|c| c.as_str())
                    ) {
                        let line_num = line as usize;

                        if line_num < lines.len() {
                            // Replace the line
                            lines[line_num] = content;
                            edits_applied += 1;

                            results.push(json!({
                                "action": "replace",
                                "line": line_num,
                                "status": "success"
                            }));
                        } else {
                            results.push(json!({
                                "action": "replace",
                                "line": line_num,
                                "status": "error",
                                "message": "Line number out of range"
                            }));
                        }
                    }
                },
                "delete" => {
                    if let Some(line) = edit.get("line").and_then(|l| l.as_u64()) {
                        let line_num = line as usize;

                        if line_num < lines.len() {
                            // Delete the line
                            lines.remove(line_num);
                            edits_applied += 1;

                            results.push(json!({
                                "action": "delete",
                                "line": line_num,
                                "status": "success"
                            }));
                        } else {
                            results.push(json!({
                                "action": "delete",
                                "line": line_num,
                                "status": "error",
                                "message": "Line number out of range"
                            }));
                        }
                    }
                },
                "region" => {
                    if let (Some(start), Some(end), Some(content)) = (
                        edit.get("start").and_then(|s| s.as_u64()),
                        edit.get("end").and_then(|e| e.as_u64()),
                        edit.get("content").and_then(|c| c.as_str())
                    ) {
                        let start_line = start as usize;
                        let end_line = end as usize;

                        if start_line <= end_line && start_line < lines.len() {
                            // Extract lines before the region
                            let before = if start_line > 0 {
                                lines[0..start_line].to_vec()
                            } else {
                                Vec::new()
                            };

                            // Extract lines after the region
                            let after = if end_line < lines.len() {
                                lines[end_line + 1..].to_vec()
                            } else {
                                Vec::new()
                            };

                            // Split the new content into lines
                            let new_lines: Vec<&str> = content.lines().collect();

                            // Combine before, new content, and after
                            let mut new_lines_vec = Vec::new();
                            new_lines_vec.extend(before);
                            new_lines_vec.extend(new_lines);
                            new_lines_vec.extend(after);

                            lines = new_lines_vec;
                            edits_applied += 1;

                            results.push(json!({
                                "action": "region",
                                "start": start_line,
                                "end": end_line,
                                "status": "success"
                            }));
                        } else {
                            results.push(json!({
                                "action": "region",
                                "start": start_line,
                                "end": end_line,
                                "status": "error",
                                "message": "Invalid line range"
                            }));
                        }
                    }
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

        // Write the updated content back to the file
        let new_content = lines.join("\n");
        self.editor.write_file(file_path, &new_content).await?;

        Ok(json!({
            "success": true,
            "action": "edit",
            "file": file_path.to_string_lossy(),
            "edits_applied": edits_applied,
            "results": results
        }))
    }

    // Create a new file
    async fn apply_create_suggestion(
        &self,
        file_path: &Path,
        suggestion: &Value,
    ) -> anyhow::Result<Value> {
        // Get content for the new file
        let content = match suggestion.get("content").and_then(|c| c.as_str()) {
            Some(c) => c,
            None => {
                return Err(SuggestionApplyError::InvalidFormat(
                    "Missing 'content' field in create suggestion".to_string()
                ).into());
            }
        };

        // Check if we should overwrite an existing file
        let overwrite = suggestion
            .get("overwrite")
            .and_then(|o| o.as_bool())
            .unwrap_or(false);

        if file_path.exists() && !overwrite {
            return Err(SuggestionApplyError::FileError(
                format!("File already exists and overwrite not specified: {}", file_path.display())
            ).into());
        }

        // Make sure parent directory exists
        if let Some(parent) = file_path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }

        // Write the content to the file
        self.editor.write_file(file_path, content).await?;

        Ok(json!({
            "success": true,
            "action": "create",
            "file": file_path.to_string_lossy(),
            "overwritten": file_path.exists()
        }))
    }
}
