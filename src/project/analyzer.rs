use crate::shared::logging;
use regex::Regex;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncReadExt;

pub struct ProjectAnalyzer {
    base_directory: PathBuf,
}

impl ProjectAnalyzer {
    pub fn new(base_directory: PathBuf) -> Self {
        Self { base_directory }
    }

    // Analyze an entire project directory
    pub async fn analyze_project(&self) -> anyhow::Result<Value> {
        logging::info(&format!(
            "Analyzing project in: {}",
            self.base_directory.display()
        ));

        // Count files by extension
        let mut extension_counts = std::collections::HashMap::new();
        let mut total_files = 0;
        let mut total_dirs = 0;
        let mut total_size = 0;

        // File types to detect (extension, language name)
        let known_types = [
            ("rs", "Rust"),
            ("go", "Go"),
            ("js", "JavaScript"),
            ("ts", "TypeScript"),
            ("py", "Python"),
            ("java", "Java"),
            ("c", "C"),
            ("cpp", "C++"),
            ("h", "C/C++ Header"),
            ("hpp", "C++ Header"),
            ("cs", "C#"),
            ("rb", "Ruby"),
            ("php", "PHP"),
            ("html", "HTML"),
            ("css", "CSS"),
            ("json", "JSON"),
            ("md", "Markdown"),
            ("yml", "YAML"),
            ("yaml", "YAML"),
            ("toml", "TOML"),
            ("xml", "XML"),
            ("txt", "Text"),
            ("sh", "Shell"),
            ("bat", "Batch"),
            ("ps1", "PowerShell"),
            ("tf", "Terraform"),
            ("sql", "SQL"),
        ];

        // Recursively process directory
        self.process_directory(
            &self.base_directory,
            &mut extension_counts,
            &mut total_files,
            &mut total_dirs,
            &mut total_size,
        )
        .await?;

        // Build result JSON
        let mut languages = Vec::new();
        for (ext, count) in &extension_counts {
            // Find language name for this extension
            let language = known_types
                .iter()
                .find(|(e, _)| e == &ext.as_str())
                .map(|(_, lang)| *lang)
                .unwrap_or("Unknown");

            languages.push(json!({
                "extension": ext,
                "language": language,
                "count": count
            }));
        }

        // Detect key files
        let key_files = self.detect_key_files().await?;

        // Detect project type
        let project_type = self.detect_project_type(&key_files).await?;

        let result = json!({
            "project_directory": self.base_directory.to_string_lossy(),
            "project_type": project_type,
            "stats": {
                "total_files": total_files,
                "total_directories": total_dirs,
                "total_size_bytes": total_size
            },
            "languages": languages,
            "key_files": key_files
        });

        Ok(result)
    }

    // Process a directory recursively
    async fn process_directory(
        &self,
        dir: &Path,
        extension_counts: &mut std::collections::HashMap<String, usize>,
        total_files: &mut usize,
        total_dirs: &mut usize,
        total_size: &mut u64,
    ) -> anyhow::Result<()> {
        let mut entries = fs::read_dir(dir).await?;

        *total_dirs += 1;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            // Skip hidden files and directories
            if path
                .file_name()
                .map(|n| n.to_string_lossy().starts_with('.'))
                .unwrap_or(false)
            {
                continue;
            }

            if path.is_dir() {
                // Recursively process subdirectory
                Box::pin(self.process_directory(
                    &path,
                    extension_counts,
                    total_files,
                    total_dirs,
                    total_size,
                ))
                .await?;
            } else if path.is_file() {
                // Process file
                *total_files += 1;

                // Get file size
                if let Ok(metadata) = fs::metadata(&path).await {
                    *total_size += metadata.len();
                }

                // Count by extension
                if let Some(ext) = path.extension() {
                    let ext_str = ext.to_string_lossy().to_lowercase();
                    *extension_counts.entry(ext_str.to_string()).or_insert(0) += 1;
                }
            }
        }

        Ok(())
    }

    // List files in the project that match a pattern
    pub async fn list_files(&self, pattern: Option<&str>) -> anyhow::Result<Vec<PathBuf>> {
        let mut results = Vec::new();

        // Compile regex if pattern is provided
        let regex = match pattern {
            Some(pattern) => Some(
                Regex::new(pattern).map_err(|e| anyhow::anyhow!("Invalid regex pattern: {}", e))?,
            ),
            None => None,
        };

        // Recursively find files
        self.find_files_recursive(&self.base_directory, &regex, &mut results)
            .await?;

        Ok(results)
    }

    // Recursively find files matching a pattern
    async fn find_files_recursive(
        &self,
        dir: &Path,
        regex: &Option<Regex>,
        results: &mut Vec<PathBuf>,
    ) -> anyhow::Result<()> {
        let mut entries = fs::read_dir(dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            // Skip hidden files and directories
            if path
                .file_name()
                .map(|n| n.to_string_lossy().starts_with('.'))
                .unwrap_or(false)
            {
                continue;
            }

            if path.is_dir() {
                // Recursively process subdirectory
                Box::pin(self.find_files_recursive(&path, regex, results)).await?;            } else if path.is_file() {
                // Check if file matches pattern
                let file_name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();

                let include = match regex {
                    Some(re) => re.is_match(&file_name),
                    None => true, // No pattern means include all files
                };

                if include {
                    results.push(path);
                }
            }
        }

        Ok(())
    }

    // Search for text in files
    pub async fn search_files(&self, query: &str) -> anyhow::Result<Value> {
        logging::info(&format!("Searching for '{}' in project", query));

        let mut results = Vec::new();
        let search_regex =
            Regex::new(query).map_err(|e| anyhow::anyhow!("Invalid search pattern: {}", e))?;

        // Find all text files
        let text_extensions = [
            "txt", "md", "rs", "go", "js", "ts", "py", "java", "c", "cpp", "h", "hpp", "cs", "rb",
            "php", "html", "css", "json", "yml", "yaml", "toml", "xml", "sh", "bat", "ps1", "tf",
            "sql",
        ];

        let mut files_to_search = Vec::new();

        // First, gather all text files
        let mut entries = fs::read_dir(&self.base_directory).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            // Skip hidden files and directories
            if path
                .file_name()
                .map(|n| n.to_string_lossy().starts_with('.'))
                .unwrap_or(false)
            {
                continue;
            }

            if path.is_dir() {
                // Recursively gather files from subdirectory
                self.gather_text_files(&path, &text_extensions, &mut files_to_search)
                    .await?;
            } else if path.is_file() {
                // Check if it's a text file
                if let Some(ext) = path.extension() {
                    let ext_str = ext.to_string_lossy().to_lowercase();
                    if text_extensions.contains(&ext_str.as_ref()) {
                        files_to_search.push(path);
                    }
                }
            }
        }

        // Now search through each file
        for file_path in files_to_search {
            let mut file = match fs::File::open(&file_path).await {
                Ok(f) => f,
                Err(_) => continue, // Skip files we can't open
            };

            let mut content = String::new();
            if file.read_to_string(&mut content).await.is_err() {
                continue; // Skip files we can't read as text
            }

            let mut line_matches = Vec::new();

            // Search line by line
            for (i, line) in content.lines().enumerate() {
                if search_regex.is_match(line) {
                    line_matches.push(json!({
                        "line_number": i + 1,
                        "line": line
                    }));
                }
            }

            if !line_matches.is_empty() {
                // Convert path to relative to base directory
                let rel_path = file_path
                    .strip_prefix(&self.base_directory)
                    .unwrap_or(&file_path)
                    .to_string_lossy();

                results.push(json!({
                    "file": rel_path,
                    "matches": line_matches
                }));
            }
        }

        Ok(json!({
            "query": query,
            "results": results
        }))
    }

    // Helper to gather text files recursively
    async fn gather_text_files(
        &self,
        dir: &Path,
        text_extensions: &[&str],
        files: &mut Vec<PathBuf>,
    ) -> anyhow::Result<()> {
        let mut entries = fs::read_dir(dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            // Skip hidden files and directories
            if path
                .file_name()
                .map(|n| n.to_string_lossy().starts_with('.'))
                .unwrap_or(false)
            {
                continue;
            }

            if path.is_dir() {
                // Recursively process subdirectory
                Box::pin(self.gather_text_files(&path, text_extensions, files)).await?;
            } else if path.is_file() {
                // Check if it's a text file
                if let Some(ext) = path.extension() {
                    let ext_str = ext.to_string_lossy().to_lowercase();
                    if text_extensions.contains(&ext_str.as_ref()) {
                        files.push(path);
                    }
                }
            }
        }

        Ok(())
    }

    // Detect key files in the project
    async fn detect_key_files(&self) -> anyhow::Result<Value> {
        // Define key files to look for
        let key_files = [
            // Version control
            ".git/config",
            ".gitignore",
            ".gitmodules",
            // Package managers
            "package.json",
            "Cargo.toml",
            "go.mod",
            "requirements.txt",
            "pom.xml",
            "build.gradle",
            // Configuration
            ".env",
            ".env.example",
            "docker-compose.yml",
            "Dockerfile",
            "Makefile",
            "CMakeLists.txt",
            // Documentation
            "README.md",
            "LICENSE",
            "CONTRIBUTING.md",
            "CHANGELOG.md",
            // CI/CD
            ".github/workflows",
            ".travis.yml",
            "Jenkinsfile",
            "azure-pipelines.yml",
        ];

        let mut found_files = Vec::new();

        for &file in &key_files {
            let file_path = self.base_directory.join(file);
            if file_path.exists() {
                let rel_path = file_path
                    .strip_prefix(&self.base_directory)
                    .unwrap_or(&file_path)
                    .to_string_lossy();

                found_files.push(json!({
                    "file": rel_path,
                    "exists": true
                }));
            }
        }

        Ok(json!(found_files))
    }

    // Detect project type based on key files
    async fn detect_project_type(&self, key_files: &Value) -> anyhow::Result<Value> {
        // Convert key_files to a vector of strings for easier checking
        let empty_list = Vec::new();
        let key_files_array = key_files.as_array().unwrap_or(&empty_list);
        let array_ref = key_files_array;
        let key_file_names: Vec<String> = array_ref.iter()
            .filter_map(|f| f.get("file").and_then(|n| n.as_str()))
            .map(|s| s.to_string())
            .collect();

        // Project type detection rules
        let project_types = [
            ("Node.js", vec!["package.json"]),
            ("Rust", vec!["Cargo.toml"]),
            ("Go", vec!["go.mod"]),
            ("Python", vec!["requirements.txt", "setup.py"]),
            ("Java", vec!["pom.xml", "build.gradle"]),
            ("C/C++", vec!["CMakeLists.txt", "Makefile"]),
            ("Docker", vec!["Dockerfile", "docker-compose.yml"]),
        ];

        let mut detected_types = Vec::new();

        for (project_type, markers) in &project_types {
            let has_markers = markers
                .iter()
                .any(|marker| key_file_names.contains(&marker.to_string()));

            if has_markers {
                detected_types.push(project_type.to_string());
            }
        }

        // If no type detected, check for common files
        if detected_types.is_empty() {
            // Count files by extension
            let mut extension_counts = std::collections::HashMap::new();
            self.count_extensions(&self.base_directory, &mut extension_counts)
                .await?;

            // Detect based on file extensions
            if extension_counts.get("rs").unwrap_or(&0) > &0 {
                detected_types.push("Rust".to_string());
            } else if extension_counts.get("py").unwrap_or(&0) > &0 {
                detected_types.push("Python".to_string());
            } else if extension_counts.get("js").unwrap_or(&0) > &0 {
                detected_types.push("JavaScript".to_string());
            } else if extension_counts.get("ts").unwrap_or(&0) > &0 {
                detected_types.push("TypeScript".to_string());
            } else if extension_counts.get("go").unwrap_or(&0) > &0 {
                detected_types.push("Go".to_string());
            } else if extension_counts.get("java").unwrap_or(&0) > &0 {
                detected_types.push("Java".to_string());
            } else if extension_counts.get("html").unwrap_or(&0) > &0 {
                detected_types.push("Web".to_string());
            } else if extension_counts.get("tf").unwrap_or(&0) > &0 {
                detected_types.push("Terraform".to_string());
            }
        }

        if detected_types.is_empty() {
            detected_types.push("Unknown".to_string());
        }

        Ok(json!(detected_types))
    }

    // Helper to count file extensions
    async fn count_extensions(
        &self,
        dir: &Path,
        counts: &mut std::collections::HashMap<String, usize>,
    ) -> anyhow::Result<()> {
        let mut entries = fs::read_dir(dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            // Skip hidden files and directories
            if path
                .file_name()
                .map(|n| n.to_string_lossy().starts_with('.'))
                .unwrap_or(false)
            {
                continue;
            }

            if path.is_dir() {
                // Recursively process subdirectory
                Box::pin(self.count_extensions(&path, counts)).await?;
            } else if path.is_file() {
                // Count by extension
                if let Some(ext) = path.extension() {
                    let ext_str = ext.to_string_lossy().to_lowercase();
                    *counts.entry(ext_str.to_string()).or_insert(0) += 1;
                }
            }
        }

        Ok(())
    }
}
