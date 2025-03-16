
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Config file not found: {0}")]
    ConfigFileNotFound(String),

    #[error("Failed to parse config file: {0}")]
    ParseError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub project: ProjectConfig,
    pub editor: EditorConfig,
    pub backups: BackupConfig,
    pub mcp: McpConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProjectConfig {
    pub directory: Option<String>,
    pub default_extension: Option<String>,
    pub exclude_patterns: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EditorConfig {
    pub tab_size: Option<usize>,
    pub indent_with_tabs: Option<bool>,
    pub line_endings: Option<String>,
    pub max_line_length: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BackupConfig {
    pub enabled: Option<bool>,
    pub max_backups_per_file: Option<usize>,
    pub backup_directory: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct McpConfig {
    pub tools: Vec<String>,
}

pub fn init_default() -> anyhow::Result<Config> {
    // Check if config exists in the default location
    let config_paths = [
        format!(
            "{}/.config/mcedit/config.json",
            std::env::var("HOME").unwrap_or_else(|_| "~".to_string())
        ),
        "./mcedit.json".to_string(),
    ];

    for path in config_paths {
        if Path::new(&path).exists() {
            return init_from_path(&path);
        }
    }

    // Return default config if no config file found
    Ok(Config {
        project: ProjectConfig {
            directory: None,
            default_extension: Some("txt".to_string()),
            exclude_patterns: Some(vec![
                ".git".to_string(),
                "node_modules".to_string(),
                "target".to_string(),
                ".backup".to_string(),
            ]),
        },
        editor: EditorConfig {
            tab_size: Some(4),
            indent_with_tabs: Some(false),
            line_endings: Some("lf".to_string()),
            max_line_length: Some(100),
        },
        backups: BackupConfig {
            enabled: Some(true),
            max_backups_per_file: Some(10),
            backup_directory: None,
        },
        mcp: McpConfig {
            tools: vec![
                "read_file".to_string(),
                "write_file".to_string(),
                "list_files".to_string(),
                "search_files".to_string(),
                "analyze_project".to_string(),
                "apply_suggestion".to_string(),
                "generate_diff".to_string(),
            ],
        },
    })
}

pub fn init_from_path(path: &str) -> anyhow::Result<Config> {
    let path = Path::new(path);

    if !path.exists() {
        return Err(ConfigError::ConfigFileNotFound(path.to_string_lossy().to_string()).into());
    }

    let content = fs::read_to_string(path)?;

    match serde_json::from_str(&content) {
        Ok(config) => Ok(config),
        Err(e) => Err(ConfigError::ParseError(e.to_string()).into()),
    }
}
