use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};
use tokio::fs::{self, File};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use thiserror::Error;
use crate::shared::logging;

// Maximum number of backups to keep per file
const MAX_BACKUPS_PER_FILE: usize = 10;

#[derive(Error, Debug)]
pub enum BackupError {
    #[error("Backup directory creation failed: {0}")]
    DirectoryCreationFailed(String),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("No backup available for: {0}")]
    NoBackupAvailable(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub struct BackupManager {
    backup_dir: PathBuf,
}

impl BackupManager {
    pub fn new(base_directory: &PathBuf) -> Result<Self, BackupError> {
        // Create a .backups directory inside the base directory
        let backup_dir = base_directory.join(".backups");

        // Ensure backup directory exists
        if !backup_dir.exists() {
            std::fs::create_dir_all(&backup_dir).map_err(|e| {
                BackupError::DirectoryCreationFailed(format!(
                    "Failed to create backup directory {}: {}",
                    backup_dir.display(),
                    e
                ))
            })?;
        }

        logging::info(&format!("Backup directory set to: {}", backup_dir.display()));

        Ok(Self { backup_dir })
    }

    // Generates a unique backup filename based on original path and timestamp
    fn generate_backup_filename(&self, path: &Path) -> Result<PathBuf, BackupError> {
        // Get the filename without the directory path
        let filename = path.file_name()
            .ok_or_else(|| BackupError::FileNotFound(
                "Invalid path: no filename component".to_string()
            ))?
            .to_string_lossy();

        // Generate a timestamp
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S%.3f");

        // Create backup filename: original_name.timestamp.bak
        let backup_filename = format!("{}_{}.bak", filename, timestamp);

        // Create a hash of the original path to use as a directory name
        // This preserves the original directory structure in a flattened way
        let path_hash = {
            let canonical_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
            let path_str = canonical_path.to_string_lossy();

            // Create a simple hash of the path
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};

            let mut hasher = DefaultHasher::new();
            path_str.hash(&mut hasher);
            format!("{:016x}", hasher.finish())
        };

        // Create backup subdirectory based on path hash
        let backup_subdir = self.backup_dir.join(path_hash);
        if !backup_subdir.exists() {
            std::fs::create_dir_all(&backup_subdir).map_err(|e| {
                BackupError::DirectoryCreationFailed(format!(
                    "Failed to create backup subdirectory {}: {}",
                    backup_subdir.display(),
                    e
                ))
            })?;
        }

        // Return the full backup path
        Ok(backup_subdir.join(backup_filename))
    }

    // Creates a backup of the specified file
    pub async fn create_backup(&self, path: &Path) -> Result<PathBuf, BackupError> {
        if !path.exists() {
            return Err(BackupError::FileNotFound(path.to_string_lossy().to_string()));
        }

        // Only backup regular files
        if !path.is_file() {
            logging::warn(&format!("Not backing up non-file: {}", path.display()));
            return Err(BackupError::FileNotFound(format!(
                "Not a regular file: {}",
                path.display()
            )));
        }

        let backup_path = self.generate_backup_filename(path)?;

        // Copy the file content
        let mut source = File::open(path).await?;
        let mut content = Vec::new();
        source.read_to_end(&mut content).await?;

        let mut destination = File::create(&backup_path).await?;
        destination.write_all(&content).await?;

        logging::info(&format!(
            "Created backup of {} at {}",
            path.display(),
            backup_path.display()
        ));

        // Clean up old backups if we have too many
        self.cleanup_old_backups(path).await?;

        Ok(backup_path)
    }

    // Lists all available backups for a file
    pub async fn list_backups(&self, path: &Path) -> Result<Vec<PathBuf>, BackupError> {
        // Get the hash directory name for this file
        let canonical_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let path_str = canonical_path.to_string_lossy();

        // Create a simple hash of the path
        let path_hash = {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};

            let mut hasher = DefaultHasher::new();
            path_str.hash(&mut hasher);
            format!("{:016x}", hasher.finish())
        };

        let backup_subdir = self.backup_dir.join(path_hash);
        if !backup_subdir.exists() {
            return Ok(Vec::new()); // No backups yet
        }

        let filename = path.file_name()
            .ok_or_else(|| BackupError::FileNotFound(
                "Invalid path: no filename component".to_string()
            ))?
            .to_string_lossy();

        // Read directory entries and filter for backups of this file
        let mut backups = Vec::new();
        let mut entries = fs::read_dir(&backup_subdir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let entry_path = entry.path();
            let entry_name = entry_path.file_name()
                .unwrap_or_default()
                .to_string_lossy();

            // Check if this is a backup for our file
            // Format is: filename_timestamp.bak
            if entry_name.starts_with(&*filename) && entry_name.ends_with(".bak") {
                backups.push(entry_path);
            }
        }

        // Sort backups by modification time (newest first)
        backups.sort_by(|a, b| {
            let a_meta = std::fs::metadata(a).ok();
            let b_meta = std::fs::metadata(b).ok();

            match (a_meta, b_meta) {
                (Some(a_m), Some(b_m)) => {
                    let b_time = b_m.modified().unwrap_or_else(|_| std::time::SystemTime::now());
                    let a_time = a_m.modified().unwrap_or_else(|_| std::time::SystemTime::now());
                    b_time.cmp(&a_time)
                },
                _ => std::cmp::Ordering::Equal,
            }
        });

        Ok(backups)
    }

    // Restores the latest backup for a file
    pub async fn restore_latest_backup(&self, path: &Path) -> Result<(), BackupError> {
        let backups = self.list_backups(path).await?;

        if backups.is_empty() {
            return Err(BackupError::NoBackupAvailable(
                path.to_string_lossy().to_string()
            ));
        }

        // Get the most recent backup (should be first after sorting)
        let latest_backup = &backups[0];

        // Read the backup content
        let mut backup_file = File::open(latest_backup).await?;
        let mut content = Vec::new();
        backup_file.read_to_end(&mut content).await?;

        // Ensure target directory exists
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).await?;
            }
        }

        // Write the content back to the original file
        let mut dest_file = File::create(path).await?;
        dest_file.write_all(&content).await?;

        logging::info(&format!(
            "Restored file {} from backup {}",
            path.display(),
            latest_backup.display()
        ));

        Ok(())
    }

    // Restores a specific backup
    pub async fn restore_specific_backup(&self, backup_path: &Path, target_path: &Path) -> Result<(), BackupError> {
        if !backup_path.exists() {
            return Err(BackupError::FileNotFound(
                backup_path.to_string_lossy().to_string()
            ));
        }

        // Read the backup content
        let mut backup_file = File::open(backup_path).await?;
        let mut content = Vec::new();
        backup_file.read_to_end(&mut content).await?;

        // Ensure target directory exists
        if let Some(parent) = target_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).await?;
            }
        }

        // Write the content back to the target file
        let mut dest_file = File::create(target_path).await?;
        dest_file.write_all(&content).await?;

        logging::info(&format!(
            "Restored file {} from specific backup {}",
            target_path.display(),
            backup_path.display()
        ));

        Ok(())
    }

    // Cleans up old backups, keeping only the most recent MAX_BACKUPS_PER_FILE
    async fn cleanup_old_backups(&self, path: &Path) -> Result<(), BackupError> {
        let backups = self.list_backups(path).await?;

        if backups.len() <= MAX_BACKUPS_PER_FILE {
            return Ok(());
        }

        // Remove older backups (everything after the max)
        for backup_to_remove in &backups[MAX_BACKUPS_PER_FILE..] {
            if let Err(e) = fs::remove_file(backup_to_remove).await {
                logging::warn(&format!(
                    "Failed to remove old backup {}: {}",
                    backup_to_remove.display(),
                    e
                ));
            } else {
                logging::info(&format!(
                    "Removed old backup: {}",
                    backup_to_remove.display()
                ));
            }
        }

        Ok(())
    }

    // Gets metadata about backups
    pub async fn get_backup_stats(&self, path: &Path) -> Result<serde_json::Value, BackupError> {
        let backups = self.list_backups(path).await?;

        // Collect metadata for each backup
        let mut backup_info = Vec::new();
        for backup in &backups {
            if let Ok(metadata) = fs::metadata(backup).await {
                let modified = metadata.modified().unwrap_or_else(|_| std::time::SystemTime::now());
                let modified_str = DateTime::<Utc>::from(modified).to_rfc3339();

                let size_bytes = metadata.len();

                backup_info.push(serde_json::json!({
                    "path": backup.to_string_lossy(),
                    "modified": modified_str,
                    "size_bytes": size_bytes
                }));
            }
        }

        Ok(serde_json::json!({
            "file": path.to_string_lossy(),
            "backup_count": backups.len(),
            "backups": backup_info
        }))
    }
}
