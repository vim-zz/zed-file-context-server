use std::path::Path;
use tokio::fs::{self, File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use thiserror::Error;
use crate::shared::logging;

#[derive(Error, Debug)]
pub enum EditorError {
    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Line number out of range: {0}")]
    LineOutOfRange(usize),

    #[error("Invalid range: {start}-{end}")]
    InvalidRange { start: usize, end: usize },
}

pub struct FileEditor {
    // Could add configuration options here in the future
}

impl FileEditor {
    pub fn new() -> Self {
        Self {}
    }

    // Basic file operations

    pub async fn read_file(&self, path: &Path) -> Result<String, EditorError> {
        if !path.exists() {
            return Err(EditorError::FileNotFound(path.to_string_lossy().to_string()));
        }

        let mut file = File::open(path).await?;
        let mut content = String::new();
        file.read_to_string(&mut content).await?;

        Ok(content)
    }

    pub async fn write_file(&self, path: &Path, content: &str) -> Result<(), EditorError> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).await?;
            }
        }

        let mut file = File::create(path).await?;
        file.write_all(content.as_bytes()).await?;

        logging::info(&format!("Wrote file: {}", path.display()));
        Ok(())
    }

    pub async fn append_to_file(&self, path: &Path, content: &str) -> Result<(), EditorError> {
        if !path.exists() {
            return Err(EditorError::FileNotFound(path.to_string_lossy().to_string()));
        }

        let mut file = OpenOptions::new()
            .append(true)
            .open(path)
            .await?;

        file.write_all(content.as_bytes()).await?;

        logging::info(&format!("Appended to file: {}", path.display()));
        Ok(())
    }

    // Line-based operations

    pub async fn insert_line(&self, path: &Path, line_num: usize, content: &str) -> Result<(), EditorError> {
        if !path.exists() {
            return Err(EditorError::FileNotFound(path.to_string_lossy().to_string()));
        }

        let file_content = self.read_file(path).await?;
        let mut lines: Vec<&str> = file_content.lines().collect();

        // If line_num is beyond the file length, we'll append to the end
        if line_num > lines.len() {
            return Err(EditorError::LineOutOfRange(line_num));
        }

        // Insert the new line at the specified position
        lines.insert(line_num, content);
        let new_content = lines.join("\n");

        self.write_file(path, &new_content).await?;

        logging::info(&format!("Inserted line {} in file: {}", line_num, path.display()));
        Ok(())
    }

    pub async fn replace_line(&self, path: &Path, line_num: usize, content: &str) -> Result<(), EditorError> {
        if !path.exists() {
            return Err(EditorError::FileNotFound(path.to_string_lossy().to_string()));
        }

        let file_content = self.read_file(path).await?;
        let mut lines: Vec<&str> = file_content.lines().collect();

        if line_num >= lines.len() {
            return Err(EditorError::LineOutOfRange(line_num));
        }

        // Replace the line at the specified position
        lines[line_num] = content;
        let new_content = lines.join("\n");

        self.write_file(path, &new_content).await?;

        logging::info(&format!("Replaced line {} in file: {}", line_num, path.display()));
        Ok(())
    }

    pub async fn delete_line(&self, path: &Path, line_num: usize) -> Result<(), EditorError> {
        if !path.exists() {
            return Err(EditorError::FileNotFound(path.to_string_lossy().to_string()));
        }

        let file_content = self.read_file(path).await?;
        let mut lines: Vec<&str> = file_content.lines().collect();

        if line_num >= lines.len() {
            return Err(EditorError::LineOutOfRange(line_num));
        }

        // Remove the line at the specified position
        lines.remove(line_num);
        let new_content = lines.join("\n");

        self.write_file(path, &new_content).await?;

        logging::info(&format!("Deleted line {} in file: {}", line_num, path.display()));
        Ok(())
    }

    pub async fn edit_region(
        &self,
        path: &Path,
        start_line: usize,
        end_line: usize,
        new_content: &str,
    ) -> Result<(), EditorError> {
        if !path.exists() {
            return Err(EditorError::FileNotFound(path.to_string_lossy().to_string()));
        }

        // Validate the range
        if start_line > end_line {
            return Err(EditorError::InvalidRange {
                start: start_line,
                end: end_line,
            });
        }

        let file_content = self.read_file(path).await?;
        let lines: Vec<&str> = file_content.lines().collect();

        if start_line >= lines.len() {
            return Err(EditorError::LineOutOfRange(start_line));
        }

        // Use the min of end_line and lines.len() to handle cases where
        // end_line is beyond the file
        let effective_end = end_line.min(lines.len());

        // Create a new content by joining:
        // 1. Lines before the start
        // 2. The new content
        // 3. Lines after the end
        let mut result = String::new();

        // Add lines before the start
        if start_line > 0 {
            result.push_str(&lines[0..start_line].join("\n"));
            result.push_str("\n");
        }

        // Add the new content
        result.push_str(new_content);

        // Add lines after the end
        if effective_end < lines.len() {
            if !result.ends_with('\n') {
                result.push_str("\n");
            }
            result.push_str(&lines[effective_end..].join("\n"));
        }

        self.write_file(path, &result).await?;

        logging::info(&format!(
            "Edited region lines {}-{} in file: {}",
            start_line,
            effective_end,
            path.display()
        ));

        Ok(())
    }
}
