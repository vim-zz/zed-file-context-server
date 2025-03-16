use crate::core::mcedit::{JsonRpcErrorCode, McEdit};
use crate::mcp::stdio::{Message, StdioTransport, Transport};
use crate::shared::logging;
use futures::StreamExt;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const TOOLS_JSON: &str = r#"{
  "tools": [
    {
      "name": "read_file",
      "description": "Read the content of a file",
      "inputSchema": {
        "type": "object",
        "properties": {
          "path": {
            "type": "string",
            "description": "Path to the file to read"
          }
        },
        "required": ["path"]
      },
      "outputSchema": {
        "type": "object",
        "properties": {
          "content": {
            "type": "string",
            "description": "Content of the file"
          },
          "path": {
            "type": "string",
            "description": "Path to the file that was read"
          }
        },
        "required": ["content", "path"]
      }
    },
    {
      "name": "write_file",
      "description": "Write content to a file",
      "inputSchema": {
        "type": "object",
        "properties": {
          "path": {
            "type": "string",
            "description": "Path to the file to write"
          },
          "content": {
            "type": "string",
            "description": "Content to write to the file"
          }
        },
        "required": ["path", "content"]
      },
      "outputSchema": {
        "type": "object",
        "properties": {
          "success": {
            "type": "boolean",
            "description": "Whether the write operation was successful"
          },
          "path": {
            "type": "string",
            "description": "Path to the file that was written"
          }
        },
        "required": ["success", "path"]
      }
    },
    {
      "name": "list_files",
      "description": "List files in the project directory that match a pattern",
      "inputSchema": {
        "type": "object",
        "properties": {
          "pattern": {
            "type": "string",
            "description": "Pattern to match files against (regex)"
          }
        }
      },
      "outputSchema": {
        "type": "object",
        "properties": {
          "files": {
            "type": "array",
            "items": {
              "type": "string"
            },
            "description": "List of file paths matching the pattern"
          }
        },
        "required": ["files"]
      }
    },
    {
      "name": "search_files",
      "description": "Search for text in files in the project",
      "inputSchema": {
        "type": "object",
        "properties": {
          "query": {
            "type": "string",
            "description": "Text to search for"
          }
        },
        "required": ["query"]
      },
      "outputSchema": {
        "type": "object",
        "properties": {
          "results": {
            "type": "array",
            "items": {
              "type": "object",
              "properties": {
                "file": {
                  "type": "string",
                  "description": "File path where match was found"
                },
                "matches": {
                  "type": "array",
                  "items": {
                    "type": "object",
                    "properties": {
                      "line_number": {
                        "type": "integer",
                        "description": "Line number where match was found"
                      },
                      "line": {
                        "type": "string",
                        "description": "Content of the line containing the match"
                      }
                    }
                  }
                }
              }
            },
            "description": "List of matches found"
          }
        },
        "required": ["results"]
      }
    },
    {
      "name": "analyze_project",
      "description": "Analyze the structure of the project",
      "inputSchema": {
        "type": "object",
        "properties": {}
      },
      "outputSchema": {
        "type": "object",
        "properties": {
          "project_directory": {
            "type": "string",
            "description": "Base directory of the project"
          },
          "project_type": {
            "type": "array",
            "items": {
              "type": "string"
            },
            "description": "Detected project types"
          },
          "stats": {
            "type": "object",
            "description": "Project statistics"
          },
          "languages": {
            "type": "array",
            "description": "Programming languages used in the project"
          },
          "key_files": {
            "type": "array",
            "description": "Important files in the project"
          }
        },
        "required": ["project_directory", "project_type"]
      }
    },
    {
      "name": "apply_suggestion",
      "description": "Apply suggested changes to a file",
      "inputSchema": {
        "type": "object",
        "properties": {
          "path": {
            "type": "string",
            "description": "Path to the file to modify"
          },
          "suggestion": {
            "type": "string",
            "description": "Suggestion text describing the changes"
          }
        },
        "required": ["path", "suggestion"]
      },
      "outputSchema": {
        "type": "object",
        "properties": {
          "success": {
            "type": "boolean",
            "description": "Whether the suggestion was applied successfully"
          },
          "action": {
            "type": "string",
            "description": "Type of action performed"
          },
          "path": {
            "type": "string",
            "description": "Path to the file that was modified"
          }
        },
        "required": ["success", "action", "path"]
      }
    },
    {
      "name": "generate_diff",
      "description": "Generate diff between original and modified text",
      "inputSchema": {
        "type": "object",
        "properties": {
          "original": {
            "type": "string",
            "description": "Original text"
          },
          "modified": {
            "type": "string",
            "description": "Modified text"
          }
        },
        "required": ["original", "modified"]
      },
      "outputSchema": {
        "type": "object",
        "properties": {
          "diff": {
            "type": "string",
            "description": "Unified diff between original and modified text"
          }
        },
        "required": ["diff"]
      }
    },
    {
      "name": "change_directory",
      "description": "Change the current working directory",
      "inputSchema": {
        "type": "object",
        "properties": {
          "directory": {
            "type": "string",
            "description": "New directory path"
          }
        },
        "required": ["directory"]
      },
      "outputSchema": {
        "type": "object",
        "properties": {
          "success": {
            "type": "boolean",
            "description": "Whether the directory change was successful"
          },
          "directory": {
            "type": "string",
            "description": "New current directory"
          }
        },
        "required": ["success", "directory"]
      }
    },
    {
      "name": "create_file",
      "description": "Create a new file with the specified content",
      "inputSchema": {
        "type": "object",
        "properties": {
          "path": {
            "type": "string",
            "description": "Path to the file to create"
          },
          "content": {
            "type": "string",
            "description": "Content to write to the file"
          }
        },
        "required": ["path", "content"]
      },
      "outputSchema": {
        "type": "object",
        "properties": {
          "success": {
            "type": "boolean",
            "description": "Whether the file was created successfully"
          },
          "path": {
            "type": "string",
            "description": "Path to the created file"
          }
        },
        "required": ["success", "path"]
      }
    },
    {
      "name": "rename_file",
      "description": "Rename or move a file",
      "inputSchema": {
        "type": "object",
        "properties": {
          "from_path": {
            "type": "string",
            "description": "Original path of the file"
          },
          "to_path": {
            "type": "string",
            "description": "New path for the file"
          }
        },
        "required": ["from_path", "to_path"]
      },
      "outputSchema": {
        "type": "object",
        "properties": {
          "success": {
            "type": "boolean",
            "description": "Whether the file was renamed successfully"
          },
          "from_path": {
            "type": "string",
            "description": "Original path of the file"
          },
          "to_path": {
            "type": "string",
            "description": "New path of the file"
          }
        },
        "required": ["success", "from_path", "to_path"]
      }
    },
    {
      "name": "delete_file",
      "description": "Delete a file",
      "inputSchema": {
        "type": "object",
        "properties": {
          "path": {
            "type": "string",
            "description": "Path to the file to delete"
          }
        },
        "required": ["path"]
      },
      "outputSchema": {
        "type": "object",
        "properties": {
          "success": {
            "type": "boolean",
            "description": "Whether the file was deleted successfully"
          },
          "path": {
            "type": "string",
            "description": "Path to the deleted file"
          }
        },
        "required": ["success", "path"]
      }
    }
  ]
}"#;

pub struct McpHandler<'a> {
    mcedit: &'a mut McEdit,
    initialized: bool,
}

impl<'a> McpHandler<'a> {
    pub fn new(mcedit: &'a mut McEdit) -> Self {
        Self {
            mcedit,
            initialized: false,
        }
    }

    pub async fn launch_mcp(&mut self, transport: &StdioTransport) -> anyhow::Result<()> {
        let mut stream = transport.receive();

        logging::info("MCP stdio transport server started. Waiting for JSON messages on stdin...");
        logging::send_log_message(
            transport,
            logging::LogLevel::Info,
            "McEdit server initialized and ready",
        )
        .await?;

        while let Some(msg_result) = stream.next().await {
            match msg_result {
                Ok(Message::Request {
                    id, method, params, ..
                }) => {
                    logging::log_both(
                        transport,
                        logging::LogLevel::Debug,
                        &format!(
                            "Got Request: id={}, method={}, params={:?}",
                            id, method, params
                        ),
                    )
                    .await?;

                    // Handle initialization request first
                    if method == "initialize" {
                        if let Err(err) = self.handle_initialize(transport, id).await {
                            logging::error(&format!("Error handling initialize request: {}", err));
                        }
                        self.initialized = true;
                        continue;
                    }

                    // For all other requests, ensure we're initialized
                    if !self.initialized {
                        self.send_error_response(
                            transport,
                            id,
                            JsonRpcErrorCode::InvalidRequest,
                            "Server not initialized. Send 'initialize' request first.".to_string(),
                        )
                        .await?;
                        continue;
                    }

                    if let Err(err) = self.handle_request(transport, id, method, params).await {
                        logging::error(&format!("Error handling request: {:?}", err));
                        self.send_error_response(
                            transport,
                            id,
                            JsonRpcErrorCode::InternalError,
                            format!("Failed to handle request: {}", err),
                        )
                        .await?;
                    }
                }
                Ok(Message::Notification { method, params, .. }) => {
                    logging::log_both(
                        transport,
                        logging::LogLevel::Debug,
                        &format!("Got Notification: method={}, params={:?}", method, params),
                    )
                    .await?;
                }
                Ok(Message::Response {
                    id, result, error, ..
                }) => {
                    logging::log_both(
                        transport,
                        logging::LogLevel::Debug,
                        &format!(
                            "Got Response: id={}, result={:?}, error={:?}",
                            id, result, error
                        ),
                    )
                    .await?;
                }
                Err(e) => {
                    logging::error(&format!("Error receiving message: {:?}", e));
                }
            }
        }

        Ok(())
    }

    async fn handle_request(
        &mut self,
        transport: &StdioTransport,
        id: u64,
        method: String,
        params: Option<serde_json::Value>,
    ) -> anyhow::Result<()> {
        match &*method {
            "initialize" => self.handle_initialize(transport, id).await?,
            "tools/list" => self.handle_tools_list(transport, id).await?,
            "tools/call" => {
                if let Some(params_val) = params {
                    self.handle_tools_call(transport, id, params_val).await?;
                }
            }
            "resources/list" => self.handle_resources_list(transport, id).await?,
            "prompts/list" => self.handle_prompts_list(transport, id).await?,
            _ => {
                self.send_error_response(
                    transport,
                    id,
                    JsonRpcErrorCode::MethodNotFound,
                    format!("Method not found: {}", method),
                )
                .await?;
            }
        }
        Ok(())
    }

    async fn handle_initialize(&self, transport: &StdioTransport, id: u64) -> anyhow::Result<()> {
        logging::info("Handling initialize request");

        // Create a properly structured capabilities response
        let response = Message::Response {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(json!({
                "capabilities": {
                    "experimental": {},
                    "prompts": { "listChanged": false },
                    "resources": { "listChanged": false, "subscribe": false },
                    "tools": { "listChanged": false }
                },
                "protocolVersion": "2024-11-05",
                "serverInfo": {
                    "name": "mcedit",
                    "version": "0.1.0"
                }
            })),
            error: None,
        };

        // Log the response for debugging
        if let Ok(json_str) = serde_json::to_string_pretty(&response) {
            logging::debug(&format!("Sending initialize response: {}", json_str));
        }

        // Send the response
        match transport.send(response).await {
            Ok(_) => {
                logging::info("Initialize response sent successfully");
                Ok(())
            }
            Err(e) => {
                logging::error(&format!("Failed to send initialize response: {}", e));
                Err(anyhow::anyhow!("Failed to send initialize response: {}", e))
            }
        }
    }

    async fn handle_tools_list(&self, transport: &StdioTransport, id: u64) -> anyhow::Result<()> {
        let tools_value: serde_json::Value =
            serde_json::from_str(TOOLS_JSON).expect("tools.json must be valid JSON");

        let response = Message::Response {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(tools_value),
            error: None,
        };

        transport.send(response).await?;
        Ok(())
    }

    async fn handle_tools_call(
        &mut self,
        transport: &StdioTransport,
        id: u64,
        params_val: serde_json::Value,
    ) -> anyhow::Result<()> {
        let name = params_val
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        logging::info(&format!("Handling tools/call for tool: {}", name));

        match name {
            "read_file" => {
                self.handle_read_file(transport, id, &params_val).await?;
            }
            "write_file" => {
                self.handle_write_file(transport, id, &params_val).await?;
            }
            "list_files" => {
                self.handle_list_files(transport, id, &params_val).await?;
            }
            "search_files" => {
                self.handle_search_files(transport, id, &params_val).await?;
            }
            "analyze_project" => {
                self.handle_analyze_project(transport, id).await?;
            }
            "apply_suggestion" => {
                self.handle_apply_suggestion(transport, id, &params_val)
                    .await?;
            }
            "generate_diff" => {
                self.handle_generate_diff(transport, id, &params_val)
                    .await?;
            }
            "change_directory" => {
                self.handle_change_directory(transport, id, &params_val)
                    .await?;
            }
            "create_file" => {
                self.handle_create_file(transport, id, &params_val).await?;
            }
            "rename_file" => {
                self.handle_rename_file(transport, id, &params_val).await?;
            }
            "delete_file" => {
                self.handle_delete_file(transport, id, &params_val).await?;
            }
            _ => {
                self.send_error_response(
                    transport,
                    id,
                    JsonRpcErrorCode::MethodNotFound,
                    format!("Tool not found: {}", name),
                )
                .await?;
            }
        }

        Ok(())
    }

    async fn handle_read_file(
        &self,
        transport: &StdioTransport,
        id: u64,
        params_val: &serde_json::Value,
    ) -> anyhow::Result<()> {
        // Get path parameter
        let path_str = match params_val
            .get("arguments")
            .and_then(|args| args.get("path"))
            .and_then(|p| p.as_str())
        {
            Some(p) => p,
            None => {
                return self
                    .send_error_response(
                        transport,
                        id,
                        JsonRpcErrorCode::InvalidParams,
                        "Missing required parameter: path".to_string(),
                    )
                    .await;
            }
        };

        let path = PathBuf::from(path_str);

        // Read the file
        match self.mcedit.read_file(&path).await {
            Ok(content) => {
                let result_json = json!({
                    "content": content,
                    "path": path.to_string_lossy()
                });
                let obj_as_str = serde_json::to_string(&result_json)?;
                self.send_text_response(transport, id, &obj_as_str).await?;
            }
            Err(err) => {
                self.send_error_response(
                    transport,
                    id,
                    JsonRpcErrorCode::InternalError,
                    format!("Failed to read file: {}", err),
                )
                .await?;
            }
        }

        Ok(())
    }

    async fn handle_write_file(
        &self,
        transport: &StdioTransport,
        id: u64,
        params_val: &serde_json::Value,
    ) -> anyhow::Result<()> {
        // Get path and content parameters
        let args = match params_val.get("arguments") {
            Some(a) => a,
            None => {
                return self
                    .send_error_response(
                        transport,
                        id,
                        JsonRpcErrorCode::InvalidParams,
                        "Missing required arguments".to_string(),
                    )
                    .await;
            }
        };

        let path_str = match args.get("path").and_then(|p| p.as_str()) {
            Some(p) => p,
            None => {
                return self
                    .send_error_response(
                        transport,
                        id,
                        JsonRpcErrorCode::InvalidParams,
                        "Missing required parameter: path".to_string(),
                    )
                    .await;
            }
        };

        let content = match args.get("content").and_then(|c| c.as_str()) {
            Some(c) => c,
            None => {
                return self
                    .send_error_response(
                        transport,
                        id,
                        JsonRpcErrorCode::InvalidParams,
                        "Missing required parameter: content".to_string(),
                    )
                    .await;
            }
        };

        let path = PathBuf::from(path_str);

        // Write to the file
        match self.mcedit.write_file(&path, content).await {
            Ok(()) => {
                let result_json = json!({
                    "success": true,
                    "path": path.to_string_lossy()
                });
                let obj_as_str = serde_json::to_string(&result_json)?;
                self.send_text_response(transport, id, &obj_as_str).await?;
            }
            Err(err) => {
                self.send_error_response(
                    transport,
                    id,
                    JsonRpcErrorCode::InternalError,
                    format!("Failed to write file: {}", err),
                )
                .await?;
            }
        }

        Ok(())
    }

    async fn handle_list_files(
        &self,
        transport: &StdioTransport,
        id: u64,
        params_val: &serde_json::Value,
    ) -> anyhow::Result<()> {
        // Get optional pattern parameter
        let pattern = params_val
            .get("arguments")
            .and_then(|args| args.get("pattern"))
            .and_then(|p| p.as_str());

        // List files
        match self.mcedit.list_files(pattern).await {
            Ok(files) => {
                // Convert file paths to strings
                let file_strings: Vec<String> = files
                    .iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect();

                let result_json = json!({ "files": file_strings });
                let obj_as_str = serde_json::to_string(&result_json)?;
                self.send_text_response(transport, id, &obj_as_str).await?;
            }
            Err(err) => {
                self.send_error_response(
                    transport,
                    id,
                    JsonRpcErrorCode::InternalError,
                    format!("Failed to list files: {}", err),
                )
                .await?;
            }
        }

        Ok(())
    }

    async fn handle_search_files(
        &self,
        transport: &StdioTransport,
        id: u64,
        params_val: &serde_json::Value,
    ) -> anyhow::Result<()> {
        // Get query parameter
        let query = match params_val
            .get("arguments")
            .and_then(|args| args.get("query"))
            .and_then(|q| q.as_str())
        {
            Some(q) => q,
            None => {
                return self
                    .send_error_response(
                        transport,
                        id,
                        JsonRpcErrorCode::InvalidParams,
                        "Missing required parameter: query".to_string(),
                    )
                    .await;
            }
        };

        // Search files
        match self.mcedit.search_files(query).await {
            Ok(results) => {
                let obj_as_str = serde_json::to_string(&results)?;
                self.send_text_response(transport, id, &obj_as_str).await?;
            }
            Err(err) => {
                self.send_error_response(
                    transport,
                    id,
                    JsonRpcErrorCode::InternalError,
                    format!("Failed to search files: {}", err),
                )
                .await?;
            }
        }

        Ok(())
    }

    async fn handle_analyze_project(
        &self,
        transport: &StdioTransport,
        id: u64,
    ) -> anyhow::Result<()> {
        // Analyze project
        match self.mcedit.analyze_project().await {
            Ok(analysis) => {
                let obj_as_str = serde_json::to_string(&analysis)?;
                self.send_text_response(transport, id, &obj_as_str).await?;
            }
            Err(err) => {
                self.send_error_response(
                    transport,
                    id,
                    JsonRpcErrorCode::InternalError,
                    format!("Failed to analyze project: {}", err),
                )
                .await?;
            }
        }

        Ok(())
    }

    async fn handle_apply_suggestion(
        &self,
        transport: &StdioTransport,
        id: u64,
        params_val: &serde_json::Value,
    ) -> anyhow::Result<()> {
        // Get path and suggestion parameters
        let args = match params_val.get("arguments") {
            Some(a) => a,
            None => {
                return self
                    .send_error_response(
                        transport,
                        id,
                        JsonRpcErrorCode::InvalidParams,
                        "Missing required arguments".to_string(),
                    )
                    .await;
            }
        };

        let path_str = match args.get("path").and_then(|p| p.as_str()) {
            Some(p) => p,
            None => {
                return self
                    .send_error_response(
                        transport,
                        id,
                        JsonRpcErrorCode::InvalidParams,
                        "Missing required parameter: path".to_string(),
                    )
                    .await;
            }
        };

        let suggestion = match args.get("suggestion").and_then(|s| s.as_str()) {
            Some(s) => s,
            None => {
                return self
                    .send_error_response(
                        transport,
                        id,
                        JsonRpcErrorCode::InvalidParams,
                        "Missing required parameter: suggestion".to_string(),
                    )
                    .await;
            }
        };

        let path = PathBuf::from(path_str);

        // Parse suggestion and apply it
        match self.mcedit.parse_suggestion(suggestion).await {
            Ok(parsed_suggestion) => match self.mcedit.apply_suggestion(&path, suggestion).await {
                Ok(result) => {
                    let obj_as_str = serde_json::to_string(&result)?;
                    self.send_text_response(transport, id, &obj_as_str).await?;
                }
                Err(err) => {
                    self.send_error_response(
                        transport,
                        id,
                        JsonRpcErrorCode::InternalError,
                        format!("Failed to apply suggestion: {}", err),
                    )
                    .await?;
                }
            },
            Err(err) => {
                self.send_error_response(
                    transport,
                    id,
                    JsonRpcErrorCode::InvalidParams,
                    format!("Failed to parse suggestion: {}", err),
                )
                .await?;
            }
        }

        Ok(())
    }

    async fn handle_generate_diff(
        &self,
        transport: &StdioTransport,
        id: u64,
        params_val: &serde_json::Value,
    ) -> anyhow::Result<()> {
        // Get original and modified parameters
        let args = match params_val.get("arguments") {
            Some(a) => a,
            None => {
                return self
                    .send_error_response(
                        transport,
                        id,
                        JsonRpcErrorCode::InvalidParams,
                        "Missing required arguments".to_string(),
                    )
                    .await;
            }
        };

        let original = match args.get("original").and_then(|o| o.as_str()) {
            Some(o) => o,
            None => {
                return self
                    .send_error_response(
                        transport,
                        id,
                        JsonRpcErrorCode::InvalidParams,
                        "Missing required parameter: original".to_string(),
                    )
                    .await;
            }
        };

        let modified = match args.get("modified").and_then(|m| m.as_str()) {
            Some(m) => m,
            None => {
                return self
                    .send_error_response(
                        transport,
                        id,
                        JsonRpcErrorCode::InvalidParams,
                        "Missing required parameter: modified".to_string(),
                    )
                    .await;
            }
        };

        // Generate diff
        match self.mcedit.generate_diff(original, modified).await {
            Ok(diff) => {
                let result_json = json!({ "diff": diff });
                let obj_as_str = serde_json::to_string(&result_json)?;
                self.send_text_response(transport, id, &obj_as_str).await?;
            }
            Err(err) => {
                self.send_error_response(
                    transport,
                    id,
                    JsonRpcErrorCode::InternalError,
                    format!("Failed to generate diff: {}", err),
                )
                .await?;
            }
        }

        Ok(())
    }

    async fn handle_change_directory(
        &mut self,
        transport: &StdioTransport,
        id: u64,
        params_val: &serde_json::Value,
    ) -> anyhow::Result<()> {
        // Get directory parameter
        let directory = match params_val
            .get("arguments")
            .and_then(|args| args.get("directory"))
            .and_then(|d| d.as_str())
        {
            Some(d) => d,
            None => {
                return self
                    .send_error_response(
                        transport,
                        id,
                        JsonRpcErrorCode::InvalidParams,
                        "Missing required parameter: directory".to_string(),
                    )
                    .await;
            }
        };

        // Change directory
        match self.mcedit.change_current_directory(directory.to_string()) {
            Ok(()) => {
                let current_dir = self.mcedit.get_current_directory();
                let result_json = json!({
                    "success": true,
                    "directory": current_dir.to_string_lossy()
                });
                let obj_as_str = serde_json::to_string(&result_json)?;
                self.send_text_response(transport, id, &obj_as_str).await?;
            }
            Err(err) => {
                self.send_error_response(
                    transport,
                    id,
                    JsonRpcErrorCode::InternalError,
                    format!("Failed to change directory: {}", err),
                )
                .await?;
            }
        }

        Ok(())
    }

    async fn handle_create_file(
        &self,
        transport: &StdioTransport,
        id: u64,
        params_val: &serde_json::Value,
    ) -> anyhow::Result<()> {
        // Get path and content parameters
        let args = match params_val.get("arguments") {
            Some(a) => a,
            None => {
                return self
                    .send_error_response(
                        transport,
                        id,
                        JsonRpcErrorCode::InvalidParams,
                        "Missing required arguments".to_string(),
                    )
                    .await;
            }
        };

        let path_str = match args.get("path").and_then(|p| p.as_str()) {
            Some(p) => p,
            None => {
                return self
                    .send_error_response(
                        transport,
                        id,
                        JsonRpcErrorCode::InvalidParams,
                        "Missing required parameter: path".to_string(),
                    )
                    .await;
            }
        };

        let content = match args.get("content").and_then(|c| c.as_str()) {
            Some(c) => c,
            None => {
                return self
                    .send_error_response(
                        transport,
                        id,
                        JsonRpcErrorCode::InvalidParams,
                        "Missing required parameter: content".to_string(),
                    )
                    .await;
            }
        };

        let path = PathBuf::from(path_str);

        // Create the file
        match self.mcedit.create_file(&path, content).await {
            Ok(()) => {
                let result_json = json!({
                    "success": true,
                    "path": path.to_string_lossy()
                });
                let obj_as_str = serde_json::to_string(&result_json)?;
                self.send_text_response(transport, id, &obj_as_str).await?;
            }
            Err(err) => {
                self.send_error_response(
                    transport,
                    id,
                    JsonRpcErrorCode::InternalError,
                    format!("Failed to create file: {}", err),
                )
                .await?;
            }
        }

        Ok(())
    }

    async fn handle_rename_file(
        &self,
        transport: &StdioTransport,
        id: u64,
        params_val: &serde_json::Value,
    ) -> anyhow::Result<()> {
        // Get from_path and to_path parameters
        let args = match params_val.get("arguments") {
            Some(a) => a,
            None => {
                return self
                    .send_error_response(
                        transport,
                        id,
                        JsonRpcErrorCode::InvalidParams,
                        "Missing required arguments".to_string(),
                    )
                    .await;
            }
        };

        let from_path_str = match args.get("from_path").and_then(|p| p.as_str()) {
            Some(p) => p,
            None => {
                return self
                    .send_error_response(
                        transport,
                        id,
                        JsonRpcErrorCode::InvalidParams,
                        "Missing required parameter: from_path".to_string(),
                    )
                    .await;
            }
        };

        let to_path_str = match args.get("to_path").and_then(|p| p.as_str()) {
            Some(p) => p,
            None => {
                return self
                    .send_error_response(
                        transport,
                        id,
                        JsonRpcErrorCode::InvalidParams,
                        "Missing required parameter: to_path".to_string(),
                    )
                    .await;
            }
        };

        let from_path = PathBuf::from(from_path_str);
        let to_path = PathBuf::from(to_path_str);

        // Rename the file
        match self.mcedit.rename_file(&from_path, &to_path).await {
            Ok(()) => {
                let result_json = json!({
                    "success": true,
                    "from_path": from_path.to_string_lossy(),
                    "to_path": to_path.to_string_lossy()
                });
                let obj_as_str = serde_json::to_string(&result_json)?;
                self.send_text_response(transport, id, &obj_as_str).await?;
            }
            Err(err) => {
                self.send_error_response(
                    transport,
                    id,
                    JsonRpcErrorCode::InternalError,
                    format!("Failed to rename file: {}", err),
                )
                .await?;
            }
        }

        Ok(())
    }

    async fn handle_delete_file(
        &self,
        transport: &StdioTransport,
        id: u64,
        params_val: &serde_json::Value,
    ) -> anyhow::Result<()> {
        // Get path parameter
        let path_str = match params_val
            .get("arguments")
            .and_then(|args| args.get("path"))
            .and_then(|p| p.as_str())
        {
            Some(p) => p,
            None => {
                return self
                    .send_error_response(
                        transport,
                        id,
                        JsonRpcErrorCode::InvalidParams,
                        "Missing required parameter: path".to_string(),
                    )
                    .await;
            }
        };

        let path = PathBuf::from(path_str);

        // Delete the file
        match self.mcedit.delete_file(&path).await {
            Ok(()) => {
                let result_json = json!({
                    "success": true,
                    "path": path.to_string_lossy()
                });
                let obj_as_str = serde_json::to_string(&result_json)?;
                self.send_text_response(transport, id, &obj_as_str).await?;
            }
            Err(err) => {
                self.send_error_response(
                    transport,
                    id,
                    JsonRpcErrorCode::InternalError,
                    format!("Failed to delete file: {}", err),
                )
                .await?;
            }
        }

        Ok(())
    }

    async fn handle_resources_list(
        &self,
        transport: &StdioTransport,
        id: u64,
    ) -> anyhow::Result<()> {
        logging::info("Handling resources/list request");

        // Create a response with an empty resources list
        let response = Message::Response {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(json!({
                "resources": []
            })),
            error: None,
        };

        // Log the response for debugging
        if let Ok(json_str) = serde_json::to_string_pretty(&response) {
            logging::debug(&format!("Sending resources/list response: {}", json_str));
        }

        // Send the response
        match transport.send(response).await {
            Ok(_) => {
                logging::info("Resources list response sent successfully");
                Ok(())
            }
            Err(e) => {
                logging::error(&format!("Failed to send resources/list response: {}", e));
                Err(e.into())
            }
        }
    }

    async fn handle_prompts_list(&self, transport: &StdioTransport, id: u64) -> anyhow::Result<()> {
        logging::info("Handling prompts/list request");

        // Create a response with an empty prompts list
        let response = Message::Response {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(json!({
                "prompts": []
            })),
            error: None,
        };

        // Log the response for debugging
        if let Ok(json_str) = serde_json::to_string_pretty(&response) {
            logging::debug(&format!("Sending prompts/list response: {}", json_str));
        }

        // Send the response
        match transport.send(response).await {
            Ok(_) => {
                logging::info("Prompts list response sent successfully");
                Ok(())
            }
            Err(e) => {
                logging::error(&format!("Failed to send prompts/list response: {}", e));
                Err(e.into())
            }
        }
    }

    async fn send_text_response(
        &self,
        transport: &StdioTransport,
        id: u64,
        text: &str,
    ) -> anyhow::Result<()> {
        logging::info(&format!("Sending text response for id {}", id));

        // Create a properly structured text response
        let response = Message::Response {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(json!({
                "content": [{
                    "type": "text",
                    "text": text
                }]
            })),
            error: None,
        };

        // Log the response for debugging
        if let Ok(json_str) = serde_json::to_string_pretty(&response) {
            logging::debug(&format!("Sending text response: {}", json_str));
        }

        // Send the response
        match transport.send(response).await {
            Ok(_) => {
                logging::info("Text response sent successfully");
                Ok(())
            }
            Err(e) => {
                logging::error(&format!("Failed to send text response: {}", e));
                Err(anyhow::anyhow!("Failed to send text response: {}", e))
            }
        }
    }

    async fn send_error_response(
        &self,
        transport: &StdioTransport,
        id: u64,
        code: JsonRpcErrorCode,
        message: String,
    ) -> anyhow::Result<()> {
        logging::warn(&format!(
            "Sending error response for id {}: {}",
            id, message
        ));

        // Create a properly structured error response
        let response = Message::Response {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(json!({
                "code": code as i32,
                "message": message
            })),
        };

        // Log the response for debugging
        if let Ok(json_str) = serde_json::to_string_pretty(&response) {
            logging::debug(&format!("Sending error response: {}", json_str));
        }

        // Send the response
        match transport.send(response).await {
            Ok(_) => {
                logging::info("Error response sent successfully");
                Ok(())
            }
            Err(e) => {
                logging::error(&format!("Failed to send error response: {}", e));
                Err(anyhow::anyhow!("Failed to send error response: {}", e))
            }
        }
    }
}
