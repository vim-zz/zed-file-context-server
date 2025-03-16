use crate::shared::logging;
use similar::{ChangeTag, TextDiff};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DiffError {
    #[error("Failed to generate diff: {0}")]
    GenerationFailed(String),
}

pub struct DiffGenerator;

impl DiffGenerator {
    // Generates a unified diff between two strings
    pub fn generate_unified_diff(original: &str, modified: &str) -> anyhow::Result<String> {
        let diff = TextDiff::from_lines(original, modified);

        let mut unified_diff = String::new();

        // Add a header
        unified_diff.push_str("--- Original\n");
        unified_diff.push_str("+++ Modified\n");

        // Generate the diff
        let mut line_num_orig = 0;
        let mut line_num_mod = 0;

        // Track the current hunk
        let mut hunk_start_orig = 0;
        let mut hunk_size_orig = 0;
        let mut hunk_start_mod = 0;
        let mut hunk_size_mod = 0;
        let mut hunk_lines = Vec::new();
        let mut in_hunk = false;

        // Process all changes
        for change in diff.iter_all_changes() {
            match change.tag() {
                ChangeTag::Equal => {
                    // Unchanged line
                    if in_hunk {
                        hunk_lines.push(format!(" {}", change));
                        hunk_size_orig += 1;
                        hunk_size_mod += 1;
                    } else {
                        // Start a new hunk if we're not in one
                        hunk_start_orig = line_num_orig;
                        hunk_start_mod = line_num_mod;
                        hunk_lines = vec![format!(" {}", change)];
                        hunk_size_orig = 1;
                        hunk_size_mod = 1;
                        in_hunk = true;
                    }
                    line_num_orig += 1;
                    line_num_mod += 1;
                }
                ChangeTag::Delete => {
                    // Deleted line
                    if !in_hunk {
                        // Start a new hunk if we're not in one
                        hunk_start_orig = line_num_orig;
                        hunk_start_mod = line_num_mod;
                        in_hunk = true;
                    }
                    hunk_lines.push(format!("-{}", change));
                    hunk_size_orig += 1;
                    line_num_orig += 1;
                }
                ChangeTag::Insert => {
                    // Inserted line
                    if !in_hunk {
                        // Start a new hunk if we're not in one
                        hunk_start_orig = line_num_orig;
                        hunk_start_mod = line_num_mod;
                        in_hunk = true;
                    }
                    hunk_lines.push(format!("+{}", change));
                    hunk_size_mod += 1;
                    line_num_mod += 1;
                }
            }

            // If we have a sufficiently large hunk, flush it
            if in_hunk && hunk_lines.len() > 3 {
                let has_changes = hunk_lines
                    .iter()
                    .any(|line| line.starts_with('+') || line.starts_with('-'));

                if has_changes {
                    // Add hunk header
                    unified_diff.push_str(&format!(
                        "@@ -{},{} +{},{} @@\n",
                        hunk_start_orig + 1,
                        hunk_size_orig,
                        hunk_start_mod + 1,
                        hunk_size_mod
                    ));

                    // Add hunk lines
                    for line in &hunk_lines {
                        unified_diff.push_str(&format!("{}\n", line));
                    }

                    unified_diff.push('\n');
                }

                // Reset hunk
                in_hunk = false;
                hunk_lines.clear();
            }
        }

        // Flush any remaining hunk
        if in_hunk && !hunk_lines.is_empty() {
            let has_changes = hunk_lines
                .iter()
                .any(|line| line.starts_with('+') || line.starts_with('-'));

            if has_changes {
                // Add hunk header
                unified_diff.push_str(&format!(
                    "@@ -{},{} +{},{} @@\n",
                    hunk_start_orig + 1,
                    hunk_size_orig,
                    hunk_start_mod + 1,
                    hunk_size_mod
                ));

                // Add hunk lines
                for line in &hunk_lines {
                    unified_diff.push_str(&format!("{}\n", line));
                }
            }
        }

        Ok(unified_diff)
    }

    // Generate a simple HTML diff for visual representation
    pub fn generate_html_diff(original: &str, modified: &str) -> anyhow::Result<String> {
        let diff = TextDiff::from_lines(original, modified);

        let mut html_diff = String::from("<pre class=\"diff\">\n");

        for change in diff.iter_all_changes() {
            let change_str = change.to_string();
            let formatted_line = html_escape::encode_text(change_str.trim_end());

            match change.tag() {
                ChangeTag::Equal => {
                    html_diff.push_str(&format!(" {}\n", formatted_line));
                }
                ChangeTag::Delete => {
                    html_diff.push_str(&format!(
                        "<span class=\"deletion\">-{}</span>\n",
                        formatted_line
                    ));
                }
                ChangeTag::Insert => {
                    html_diff.push_str(&format!(
                        "<span class=\"insertion\">+{}</span>\n",
                        formatted_line
                    ));
                }
            }
        }

        html_diff.push_str("</pre>");

        Ok(html_diff)
    }

    // Generate a word-level diff that shows exactly which words changed
    pub fn generate_word_diff(original: &str, modified: &str) -> anyhow::Result<String> {
        let diff = TextDiff::from_words(original, modified);

        let mut word_diff = String::new();

        for change in diff.iter_all_changes() {
            match change.tag() {
                ChangeTag::Equal => {
                    word_diff.push_str(&change.to_string());
                }
                ChangeTag::Delete => {
                    word_diff.push_str("[-");
                    word_diff.push_str(&change.to_string());
                    word_diff.push_str("-]");
                }
                ChangeTag::Insert => {
                    word_diff.push_str("{+");
                    word_diff.push_str(&change.to_string());
                    word_diff.push_str("+}");
                }
            }
        }

        Ok(word_diff)
    }
}
