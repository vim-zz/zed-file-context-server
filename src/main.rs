
mod config;
mod core;
mod diff;
mod editor;
mod file_service;
mod mcp;
mod project;
mod shared;
mod suggestions;

use clap::{arg, command, Parser, Subcommand};
use core::mcedit::McEdit;
use shared::logging;
use std::path::PathBuf;

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(
    name = "mcedit",
    about = "✨ A CLI tool for smart file editing with AI assistance through the Model Context Protocol (MCP).",
    version = APP_VERSION,
    disable_version_flag(true)
)]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(
        long,
        short = 'c',
        value_name = "PATH",
        help = "Path to the configuration file"
    )]
    pub config: Option<String>,

    #[arg(
        long,
        short = 'd',
        value_name = "PATH",
        help = "Project directory to work with"
    )]
    pub dir: Option<String>,

    #[arg(long, short = 'V', help = "Print version")]
    pub version: bool,
}

#[derive(Subcommand)]
enum Commands {
    #[command(name = "mcp", about = "Launch mcedit as an MCP server")]
    Mcp,

    #[command(name = "edit", about = "Edit a file with the given content")]
    Edit {
        #[arg(help = "Path to the file to edit")]
        path: String,

        #[arg(help = "Content to write to the file")]
        content: Option<String>,
    },

    #[command(name = "list", about = "List files in the project")]
    List {
        #[arg(help = "Pattern to match files against (regex)")]
        pattern: Option<String>,
    },

    #[command(name = "analyze", about = "Analyze the project structure")]
    Analyze,

    #[command(name = "search", about = "Search for text in project files")]
    Search {
        #[arg(help = "Text to search for")]
        query: String,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if cli.version {
        println!("{}", APP_VERSION);
        std::process::exit(0);
    }

    match &cli.command {
        Some(cmd) => match cmd {
            Commands::Mcp => {
                logging::info("Starting mcedit in MCP server mode");
                match init_mcedit(&cli).await {
                    Ok(mut mcedit) => {
                        if let Err(err) = mcedit.launch_mcp().await {
                            logging::error(&format!("Error launching MCP server: {:?}", err));
                            std::process::exit(1);
                        }
                    }
                    Err(e) => {
                        logging::error(&format!("Failed to initialize mcedit: {}", e));
                        std::process::exit(1);
                    }
                }
            }
            Commands::Edit { path, content } => {
                logging::info(&format!("Editing file: {}", path));
                match init_mcedit(&cli).await {
                    Ok(mcedit) => {
                        let file_path = PathBuf::from(path);

                        if let Some(content_str) = content {
                            // Write content to file
                            if let Err(err) = mcedit.write_file(&file_path, content_str).await {
                                logging::error(&format!("Error writing to file: {:?}", err));
                                std::process::exit(1);
                            }
                            println!("File {} updated successfully", path);
                        } else {
                            // Read and display file content
                            match mcedit.read_file(&file_path).await {
                                Ok(content) => {
                                    println!("{}", content);
                                }
                                Err(err) => {
                                    logging::error(&format!("Error reading file: {:?}", err));
                                    std::process::exit(1);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        logging::error(&format!("Failed to initialize mcedit: {}", e));
                        std::process::exit(1);
                    }
                }
            }
            Commands::List { pattern } => {
                logging::info("Listing files in project");
                match init_mcedit(&cli).await {
                    Ok(mcedit) => {
                        match mcedit.list_files(pattern.as_deref()).await {
                            Ok(files) => {
                                for file in files {
                                    println!("{}", file.display());
                                }
                            }
                            Err(err) => {
                                logging::error(&format!("Error listing files: {:?}", err));
                                std::process::exit(1);
                            }
                        }
                    }
                    Err(e) => {
                        logging::error(&format!("Failed to initialize mcedit: {}", e));
                        std::process::exit(1);
                    }
                }
            }
            Commands::Analyze => {
                logging::info("Analyzing project structure");
                match init_mcedit(&cli).await {
                    Ok(mcedit) => {
                        match mcedit.analyze_project().await {
                            Ok(analysis) => {
                                println!("{}", serde_json::to_string_pretty(&analysis).unwrap());
                            }
                            Err(err) => {
                                logging::error(&format!("Error analyzing project: {:?}", err));
                                std::process::exit(1);
                            }
                        }
                    }
                    Err(e) => {
                        logging::error(&format!("Failed to initialize mcedit: {}", e));
                        std::process::exit(1);
                    }
                }
            }
            Commands::Search { query } => {
                logging::info(&format!("Searching for: {}", query));
                match init_mcedit(&cli).await {
                    Ok(mcedit) => {
                        match mcedit.search_files(query).await {
                            Ok(results) => {
                                println!("{}", serde_json::to_string_pretty(&results).unwrap());
                            }
                            Err(err) => {
                                logging::error(&format!("Error searching files: {:?}", err));
                                std::process::exit(1);
                            }
                        }
                    }
                    Err(e) => {
                        logging::error(&format!("Failed to initialize mcedit: {}", e));
                        std::process::exit(1);
                    }
                }
            }
        },
        None => {
            // Default behavior if no command is specified
            println!("No command specified. Use --help for usage information.");
        }
    };
}

async fn init_mcedit(cli: &Cli) -> anyhow::Result<McEdit> {
    let config_path = cli.config.clone();
    let dir_path = cli.dir.clone();

    logging::info(&format!(
        "Initializing mcedit with config: {:?}, dir: {:?}",
        config_path, dir_path
    ));
    McEdit::new(config_path, dir_path)
}
