use serde_json::{json, Value};
use thiserror::Error;
use crate::shared::logging;
use regex::Regex;

#[derive(Error, Debug)]
pub enum SuggestionParseError {
    #[error("Invalid suggestion format: {0}")]
    InvalidFormat(String),

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Invalid JSON: {0}")]
    InvalidJson(String),

    #[error("Unsupported suggestion type: {0}")]
    UnsupportedType(String),
}

pub struct SuggestionParser;

impl SuggestionParser {
    // Main function to parse a suggestion from the model
    pub fn parse_suggestion(suggestion: &str) -> anyhow::Result<Value> {
        logging::info("Parsing model suggestion");

        // First, try to parse as JSON directly
        if let Ok(json_value) = serde_json::from_str::<Value>(suggestion) {
            if Self::is_valid_json_suggestion(&json_value) {
                return Ok(json_value);
            }
        }

        // If not valid JSON, try to extract code blocks and other formats

        // Try to extract markdown-style code blocks
        if let Some(json_suggestion) = Self::extract_code_blocks(suggestion) {
            if let Ok(json_value) = serde_json::from_str::<Value>(&json_suggestion) {
                if Self::is_valid_json_suggestion(&json_value) {
                    return Ok(json_value);
                }
            }
        }

        // Try to extract file-edit-style suggestions
        if let Some(edit_suggestion) = Self::parse_file_edit_format(suggestion) {
            return Ok(edit_suggestion);
        }

        // Try to extract simple replacements
        if let Some(replacement) = Self::parse_simple_replacement(suggestion) {
            return Ok(replacement);
        }

        // If nothing else works, create a simple "replace" suggestion
        Ok(json!({
            "type": "replace",
            "content": suggestion
        }))
    }

    // Check if a JSON value is a valid suggestion format
    fn is_valid_json_suggestion(value: &Value) -> bool {
        // Must be an object
        if !value.is_object() {
            return false;
        }

        // Must have a "type" field
        let suggestion_type = match value.get("type").and_then(|t| t.as_str()) {
            Some(t) => t,
            None => return false,
        };

        // Check required fields based on type
        match suggestion_type {
            "replace" => value.get("content").is_some(),
            "edit" => value.get("edits").is_some() && value.get("edits").unwrap().is_array(),
            "create" => value.get("content").is_some(),
            _ => false,
        }
    }

    // Extract code blocks from markdown-style suggestions
    fn extract_code_blocks(text: &str) -> Option<String> {
        // Look for ```json or ```javascript code blocks
        let re = Regex::new(r"```(?:json|javascript)\s*\n([\s\S]*?)\n\s*```").ok()?;

        if let Some(captures) = re.captures(text) {
            if captures.len() > 1 {
                return Some(captures[1].to_string());
            }
        }

        // Try for any code block if specific language blocks weren't found
        let generic_re = Regex::new(r"```\s*\n([\s\S]*?)\n\s*```").ok()?;

        if let Some(captures) = generic_re.captures(text) {
            if captures.len() > 1 {
                return Some(captures[1].to_string());
            }
        }

        None
    }

    // Parse file-edit-style suggestions (like "change lines 10-20 to...")
    fn parse_file_edit_format(text: &str) -> Option<Value> {
        // Look for patterns like "replace lines X-Y with" or "insert at line X"

        // Replace range of lines
        let replace_re = Regex::new(r"(?i)(?:replace|change|modify)\s+lines?\s+(\d+)(?:\s*-\s*|\s+to\s+)(\d+)(?:\s+with)?:?\s*\n([\s\S]+)").ok()?;

        if let Some(captures) = replace_re.captures(text) {
            if captures.len() > 3 {
                let start: usize = captures[1].parse().unwrap_or(0);
                let end: usize = captures[2].parse().unwrap_or(0);
                let content = captures[3].trim();

                return Some(json!({
                    "type": "edit",
                    "edits": [{
                        "action": "region",
                        "start": start,
                        "end": end,
                        "content": content
                    }]
                }));
            }
        }

        // Insert at line
        let insert_re = Regex::new(r"(?i)(?:insert|add)\s+(?:at|after|before)\s+lines?\s+(\d+):?\s*\n([\s\S]+)").ok()?;

        if let Some(captures) = insert_re.captures(text) {
            if captures.len() > 2 {
                let line: usize = captures[1].parse().unwrap_or(0);
                let content = captures[2].trim();

                return Some(json!({
                    "type": "edit",
                    "edits": [{
                        "action": "insert",
                        "line": line,
                        "content": content
                    }]
                }));
            }
        }

        // Delete lines
        let delete_re = Regex::new(r"(?i)(?:delete|remove)\s+lines?\s+(\d+)(?:\s*-\s*|\s+to\s+)?(\d+)?").ok()?;

        if let Some(captures) = delete_re.captures(text) {
            let start: usize = captures[1].parse().unwrap_or(0);
            let end: usize = if captures.len() > 2 && !captures[2].is_empty() {
                captures[2].parse().unwrap_or(start)
            } else {
                start
            };

            let mut edits: Vec<Value> = Vec::new();

            // If deleting multiple lines, use region edit
            if start != end {
                return Some(json!({
                    "type": "edit",
                    "edits": [{
                        "action": "region",
                        "start": start,
                        "end": end,
                        "content": ""
                    }]
                }));
            } else {
                // Single line deletion
                return Some(json!({
                    "type": "edit",
                    "edits": [{
                        "action": "delete",
                        "line": start
                    }]
                }));
            }
        }

        None
    }

    // Parse simple replacements like "replace the file with..."
    fn parse_simple_replacement(text: &str) -> Option<Value> {
        // Look for "replace the file with" or "update the entire file to"
        let replace_re = Regex::new(r"(?i)(?:replace the (?:file|content)|update the entire file)(?:\s+with|\s+to)?:?\s*\n([\s\S]+)").ok()?;

        if let Some(captures) = replace_re.captures(text) {
            if captures.len() > 1 {
                let content = captures[1].trim();

                return Some(json!({
                    "type": "replace",
                    "content": content
                }));
            }
        }

        // Look for "create a new file with" or "make a file with"
        let create_re = Regex::new(r"(?i)(?:create a new file|make a file)(?:\s+with|\s+containing)?:?\s*\n([\s\S]+)").ok()?;

        if let Some(captures) = create_re.captures(text) {
            if captures.len() > 1 {
                let content = captures[1].trim();

                return Some(json!({
                    "type": "create",
                    "content": content
                }));
            }
        }

        None
    }

    // Helper for normalizing line numbers (1-based to 0-based)
    pub fn normalize_line_number(line: usize) -> usize {
        if line > 0 {
            line - 1
        } else {
            0
        }
    }
}
