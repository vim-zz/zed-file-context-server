
use crate::config::{self, Config};
use crate::diff::generator::DiffGenerator;
use crate::editor::file_editor::FileEditor;
use crate::file_service::service::FileService;
use crate::mcp::handler::McpHandler;
use crate::mcp::stdio::StdioTransport;
use crate::project::analyzer::ProjectAnalyzer;
use crate::shared::logging;
use crate::suggestions::parser::SuggestionParser;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum McEditError {
    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Invalid directory: {0}")]
    InvalidDirectory(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum JsonRpcErrorCode {
    ParseError = -32700,
    InvalidRequest = -32600,
    MethodNotFound = -32601,
    InvalidParams = -32602,
    InternalError = -32603,
    // Custom error codes should be in the range -32000 to -32099
    FileNotFound = -32000,
    PermissionDenied = -32001,
    InvalidPath = -32002,
    DiffError = -32003,
}

pub struct McEdit {
    #[allow(dead_code)]
    config: Config,
    file_service: FileService,
    project_analyzer: ProjectAnalyzer,
    current_directory: PathBuf,
}

impl McEdit {
    pub fn new(config_path: Option<String>, project_dir: Option<String>) -> anyhow::Result<Self> {
        // Check environment variable for project directory first
        let env_project_dir = std::env::var("PROJECT_DIR").ok();
        if let Some(dir) = &env_project_dir {
            logging::info(&format!(
                "Found PROJECT_DIR environment variable: {}",
                dir
            ));
        }

        // Initialize config
        let config = match config_path {
            Some(path) => {
                let path_buf = PathBuf::from(&path);
                if path_buf.is_absolute() {
                    logging::info(&format!("Using absolute config path: {}", path));
                    config::init_from_path(&path)?
                } else {
                    // Convert to absolute path
                    let abs_path = std::env::current_dir()?.join(&path);
                    logging::info(&format!(
                        "Converting relative config path to absolute: {}",
                        abs_path.display()
                    ));
                    config::init_from_path(abs_path.to_str().unwrap_or(&path))?
                }
            }
            None => {
                logging::info("No config path provided, using default configuration");
                config::init_default()?
            }
        };

        // Priority for project directory:
        // 1. Command line argument
        // 2. Environment variable
        // 3. Config file
        // 4. Current directory
        let project_directory = match project_dir {
            Some(dir) => {
                let dir_buf = PathBuf::from(&dir);
                if dir_buf.is_absolute() {
                    logging::info(&format!(
                        "Using absolute project directory from CLI arg: {}",
                        dir
                    ));
                    dir_buf
                } else {
                    // Convert to absolute path
                    let abs_dir = std::env::current_dir()?.join(dir);
                    logging::info(&format!(
                        "Converting relative project directory from CLI to absolute: {}",
                        abs_dir.display()
                    ));
                    abs_dir
                }
            }
            None => {
                match env_project_dir {
                    Some(dir) => {
                        logging::info(&format!(
                            "Using project directory from PROJECT_DIR env var: {}",
                            dir
                        ));
                        PathBuf::from(dir)
                    }
                    None => {
                        match &config.project.directory {
                            Some(dir) => {
                                let dir_buf = PathBuf::from(dir);
                                if dir_buf.is_absolute() {
                                    logging::info(&format!(
                                        "Using project directory from config: {}",
                                        dir
                                    ));
                                    dir_buf
                                } else {
                                    // Convert to absolute path
                                    let abs_dir = std::env::current_dir()?.join(dir);
                                    logging::info(&format!("Converting relative project directory from config to absolute: {}", abs_dir.display()));
                                    abs_dir
                                }
                            }
                            None => {
                                // If we're in root (/) directory, use HOME directory as fallback
                                let current_dir = std::env::current_dir()?;
                                if current_dir == PathBuf::from("/") {
                                    // We're likely running from a container or restricted environment
                                    let home_dir = dirs::home_dir().unwrap_or(current_dir.clone());
                                    let project_dir = home_dir.join("project");
                                    logging::info(&format!("Working directory is root (/), falling back to home directory: {}", project_dir.display()));
                                    project_dir
                                } else {
                                    logging::info(&format!("No project directory specified, using current directory: {}", current_dir.display()));
                                    current_dir
                                }
                            }
                        }
                    }
                }
            }
        };

        // Create directory if it doesn't exist
        if !project_directory.exists() {
            logging::info(&format!(
                "Creating project directory: {}",
                project_directory.display()
            ));
            std::fs::create_dir_all(&project_directory)?;
        }

        // Create file service and project analyzer
        let file_service = FileService::new(&project_directory)?;
        let project_analyzer = ProjectAnalyzer::new(project_directory.clone());

        logging::info("McEdit initialized successfully");
        Ok(Self {
            config,
            file_service,
            project_analyzer,
            current_directory: project_directory,
        })
    }

    pub async fn launch_mcp(&mut self) -> anyhow::Result<()> {
        let (transport, _sender) = StdioTransport::new();

        // Log environment information
        let cwd = std::env::current_dir()?;
        logging::info(&format!("Current working directory: {}", cwd.display()));

        // Check if PROJECT_DIR environment variable is set, and if not, set it
        if std::env::var("PROJECT_DIR").is_err() {
            let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
            let default_project_dir = home_dir.join("project");

            // Create the directory if it doesn't exist
            if !default_project_dir.exists() {
                logging::info(&format!(
                    "Creating default project directory: {}",
                    default_project_dir.display()
                ));
                std::fs::create_dir_all(&default_project_dir)?;
            }

            // Create a sample README.md file to help users get started
            let readme_path = default_project_dir.join("README.md");
            if !readme_path.exists() {
                logging::info(&format!(
                    "Creating sample README.md file at: {}",
                    readme_path.display()
                ));
                let sample_content = r#"# Sample Project

This is a sample project created by McEdit.

## Getting Started

You can edit this file or create new files in this directory.

## Features

- Edit any text file
- Analyze project structure
- Apply suggested changes
"#;
                std::fs::write(&readme_path, sample_content)?;
            }

            // Set the environment variable for future uses in this process
            std::env::set_var(
                "PROJECT_DIR",
                default_project_dir.to_string_lossy().to_string(),
            );
            logging::info(&format!(
                "Set PROJECT_DIR to: {}",
                default_project_dir.display()
            ));
        }

        // Create the handler and launch MCP
        let mut handler = McpHandler::new(self);
        handler.launch_mcp(&transport).await
    }

    // File operations

    pub async fn read_file(&self, path: &Path) -> anyhow::Result<String> {
        self.file_service.read_file(path).await
    }

    pub async fn write_file(&self, path: &Path, content: &str) -> anyhow::Result<()> {
        self.file_service.write_file(path, content).await
    }

    pub async fn append_to_file(&self, path: &Path, content: &str) -> anyhow::Result<()> {
        self.file_service.append_to_file(path, content).await
    }

    pub async fn edit_file_region(
        &self,
        path: &Path,
        start_line: usize,
        end_line: usize,
        new_content: &str,
    ) -> anyhow::Result<()> {
        self.file_service
            .edit_region(path, start_line, end_line, new_content)
            .await
    }

    pub async fn delete_file(&self, path: &Path) -> anyhow::Result<()> {
        self.file_service.delete_file(path).await
    }

    pub async fn rename_file(&self, from_path: &Path, to_path: &Path) -> anyhow::Result<()> {
        self.file_service.rename_file(from_path, to_path).await
    }

    pub async fn create_file(&self, path: &Path, content: &str) -> anyhow::Result<()> {
        self.file_service.create_file(path, content).await
    }

    // Project operations

    pub async fn analyze_project(&self) -> anyhow::Result<serde_json::Value> {
        self.project_analyzer.analyze_project().await
    }

    pub async fn list_files(&self, pattern: Option<&str>) -> anyhow::Result<Vec<PathBuf>> {
        self.project_analyzer.list_files(pattern).await
    }

    pub async fn search_files(&self, query: &str) -> anyhow::Result<serde_json::Value> {
        self.project_analyzer.search_files(query).await
    }

    // Diff operations

    pub async fn generate_diff(
        &self,
        original_content: &str,
        modified_content: &str,
    ) -> anyhow::Result<String> {
        DiffGenerator::generate_unified_diff(original_content, modified_content)
    }

    pub async fn preview_file_changes(
        &self,
        path: &Path,
        new_content: &str,
    ) -> anyhow::Result<String> {
        let original_content = self.read_file(path).await?;
        self.generate_diff(&original_content, new_content).await
    }

    // Suggestion operations

    pub async fn parse_suggestion(&self, suggestion: &str) -> anyhow::Result<serde_json::Value> {
        SuggestionParser::parse_suggestion(suggestion)
    }

    pub async fn apply_suggestion(
        &self,
        path: &Path,
        suggestion: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let parsed = SuggestionParser::parse_suggestion(suggestion)?;
        self.file_service.apply_suggestion(path, &parsed).await
    }

    // Directory operations

    pub fn change_current_directory(&mut self, new_directory: String) -> anyhow::Result<()> {
        let dir_path = PathBuf::from(new_directory);
        let project_directory = if dir_path.is_absolute() {
            logging::info(&format!(
                "Changing to absolute project directory: {}",
                dir_path.display()
            ));
            dir_path
        } else {
            // Convert relative path to absolute
            let abs_dir = self.current_directory.join(dir_path);
            logging::info(&format!(
                "Converting relative project directory to absolute: {}",
                abs_dir.display()
            ));
            abs_dir
        };

        // Create directory if it doesn't exist
        if !project_directory.exists() {
            logging::info(&format!(
                "Creating directory: {}",
                project_directory.display()
            ));
            std::fs::create_dir_all(&project_directory)?;
        }

        // Update file service with new directory
        self.file_service.change_directory(&project_directory)?;

        // Update project analyzer
        self.project_analyzer = ProjectAnalyzer::new(project_directory.clone());

        // Update current directory
        self.current_directory = project_directory.clone();

        // Update environment variable
        std::env::set_var(
            "PROJECT_DIR",
            project_directory.to_string_lossy().to_string(),
        );

        logging::info(&format!(
            "Successfully changed project directory to: {}",
            project_directory.display()
        ));

        Ok(())
    }

    pub fn get_current_directory(&self) -> PathBuf {
        self.current_directory.clone()
    }
}
