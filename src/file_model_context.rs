
use serde::Deserialize;
use std::env;
use zed::settings::ContextServerSettings;
use zed_extension_api::{self as zed, serde_json, Command, ContextServerId, Project, Result};

const PACKAGE_NAME: &str = "@yourusername/mcedit-context-server";
const PACKAGE_VERSION: &str = "0.1.0";
const SERVER_PATH: &str = "target/release/mcedit"; // Path to your mcedit binary

struct MceditModelContextExtension;

#[derive(Debug, Deserialize)]
struct MceditContextServerSettings {
    project_dir: Option<String>,
    log_level: Option<String>,
}

impl zed::Extension for MceditModelContextExtension {
    fn new() -> Self {
        Self
    }

    fn context_server_command(
        &mut self,
        _context_server_id: &ContextServerId,
        project: &Project,
    ) -> Result<Command> {
        // Check if the mcedit binary exists or needs to be built
        let mcedit_path = env::current_dir()
            .unwrap()
            .join(SERVER_PATH)
            .to_string_lossy()
            .to_string();

        // Get settings for the context server from Zed
        let settings = ContextServerSettings::for_project("mcedit-context-server", project)?;
        let mut project_dir = None;
        let mut log_level = None;

        if let Some(settings_value) = settings.settings {
            let settings: MceditContextServerSettings =
                serde_json::from_value(settings_value).map_err(|e| e.to_string())?;
            project_dir = settings.project_dir;
            log_level = settings.log_level;
        }

        let project_dir = project_dir.unwrap();

        // Set up environment variables for mcedit
        let mut env_vars = vec![
            ("PROJECT_DIR".into(), project_dir),
        ];

        if let Some(level) = log_level {
            env_vars.push(("MCEDIT_LOG_LEVEL".into(), level));
        }

        // Return the command to launch mcedit in MCP server mode
        Ok(Command {
            command: mcedit_path,
            args: vec!["mcp".to_string()],
            env: env_vars,
        })
    }
}

zed::register_extension!(MceditModelContextExtension);
