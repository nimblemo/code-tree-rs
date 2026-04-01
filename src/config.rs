use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

/// Application configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct Config {
    /// Project name
    pub project_name: Option<String>,

    /// Project path
    pub project_path: PathBuf,

    /// Output path
    pub output_path: PathBuf,

    /// Internal working directory path (.tree)
    pub internal_path: PathBuf,

    /// Maximum recursion depth
    pub max_depth: u8,

    /// Maximum file size limit (bytes)
    pub max_file_size: u64,

    /// Whether to include test files
    pub include_tests: bool,

    /// Whether to include hidden files
    pub include_hidden: bool,

    /// Directories to exclude
    pub excluded_dirs: Vec<String>,

    /// Files to exclude
    pub excluded_files: Vec<String>,

    /// File extensions to exclude
    pub excluded_extensions: Vec<String>,

    /// Only include specified file extensions
    pub included_extensions: Vec<String>,

    /// Cache configuration
    pub cache: CacheConfig,

    /// Whether to enable verbose output
    #[serde(default)]
    pub verbose: bool,
}

/// Cache configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct CacheConfig {
    /// Whether to enable cache
    pub enabled: bool,

    /// Cache directory
    pub cache_dir: PathBuf,

    /// Cache expiration time (hours)
    pub expire_hours: u64,

    /// Maximum parallel operations
    pub max_parallels: usize,
}

impl Config {
    /// Load configuration from file
    pub fn from_file(path: &PathBuf) -> Result<Self> {
        let mut file =
            File::open(path).context(format!("Failed to open config file: {:?}", path))?;
        let mut content = String::new();
        file.read_to_string(&mut content)
            .context("Failed to read config file")?;

        let config: Config = toml::from_str(&content).context("Failed to parse config file")?;
        Ok(config)
    }

    /// Get project name, prioritize configured project_name, otherwise auto-infer
    pub fn get_project_name(&self) -> String {
        // Prioritize configured project name
        if let Some(ref name) = self.project_name {
            if !name.trim().is_empty() {
                return name.clone();
            }
        }

        // If not configured or empty, auto-infer
        self.infer_project_name()
    }

    /// Auto-infer project name
    fn infer_project_name(&self) -> String {
        // Try to extract project name from project configuration files
        if let Some(name) = self.extract_project_name_from_config_files() {
            return name;
        }

        // Infer from project path
        self.project_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    }

    /// Extract project name from project configuration files
    fn extract_project_name_from_config_files(&self) -> Option<String> {
        // Try to extract from Cargo.toml (Rust project)
        if let Some(name) = self.extract_from_cargo_toml() {
            return Some(name);
        }

        // Try to extract from package.json (Node.js project)
        if let Some(name) = self.extract_from_package_json() {
            return Some(name);
        }

        // Try to extract from pyproject.toml (Python project)
        if let Some(name) = self.extract_from_pyproject_toml() {
            return Some(name);
        }

        // Try to extract from pom.xml (Java Maven project)
        if let Some(name) = self.extract_from_pom_xml() {
            return Some(name);
        }

        // Try to extract from .csproj (C# project)
        if let Some(name) = self.extract_from_csproj() {
            return Some(name);
        }

        None
    }

    /// Extract project name from Cargo.toml
    pub fn extract_from_cargo_toml(&self) -> Option<String> {
        let cargo_path = self.project_path.join("Cargo.toml");
        if !cargo_path.exists() {
            return None;
        }

        match std::fs::read_to_string(&cargo_path) {
            Ok(content) => {
                // Find name under [package] section
                let mut in_package_section = false;
                for line in content.lines() {
                    let line = line.trim();
                    if line == "[package]" {
                        in_package_section = true;
                        continue;
                    }
                    if line.starts_with('[') && in_package_section {
                        in_package_section = false;
                        continue;
                    }
                    if in_package_section && line.starts_with("name") && line.contains("=") {
                        if let Some(name_part) = line.split('=').nth(1) {
                            let name = name_part.trim().trim_matches('"').trim_matches('\'');
                            if !name.is_empty() {
                                return Some(name.to_string());
                            }
                        }
                    }
                }
            }
            Err(_) => return None,
        }
        None
    }

    /// Extract project name from package.json
    pub fn extract_from_package_json(&self) -> Option<String> {
        let package_path = self.project_path.join("package.json");
        if !package_path.exists() {
            return None;
        }

        match std::fs::read_to_string(&package_path) {
            Ok(content) => {
                // Simple JSON parsing, find "name": "..."
                for line in content.lines() {
                    let line = line.trim();
                    if line.starts_with("\"name\"") && line.contains(":") {
                        if let Some(name_part) = line.split(':').nth(1) {
                            let name = name_part
                                .trim()
                                .trim_matches(',')
                                .trim_matches('"')
                                .trim_matches('\'');
                            if !name.is_empty() {
                                return Some(name.to_string());
                            }
                        }
                    }
                }
            }
            Err(_) => return None,
        }
        None
    }

    /// Extract project name from pyproject.toml
    pub fn extract_from_pyproject_toml(&self) -> Option<String> {
        let pyproject_path = self.project_path.join("pyproject.toml");
        if !pyproject_path.exists() {
            return None;
        }

        match std::fs::read_to_string(&pyproject_path) {
            Ok(content) => {
                // Find name under [project] or [tool.poetry]
                let mut in_project_section = false;
                let mut in_poetry_section = false;

                for line in content.lines() {
                    let line = line.trim();
                    if line == "[project]" {
                        in_project_section = true;
                        in_poetry_section = false;
                        continue;
                    }
                    if line == "[tool.poetry]" {
                        in_poetry_section = true;
                        in_project_section = false;
                        continue;
                    }
                    if line.starts_with('[') && (in_project_section || in_poetry_section) {
                        in_project_section = false;
                        in_poetry_section = false;
                        continue;
                    }
                    if (in_project_section || in_poetry_section)
                        && line.starts_with("name")
                        && line.contains("=")
                    {
                        if let Some(name_part) = line.split('=').nth(1) {
                            let name = name_part.trim().trim_matches('"').trim_matches('\'');
                            if !name.is_empty() {
                                return Some(name.to_string());
                            }
                        }
                    }
                }
            }
            Err(_) => return None,
        }
        None
    }

    /// Extract project name from .csproj
    fn extract_from_csproj(&self) -> Option<String> {
        // Find all .csproj files
        if let Ok(entries) = std::fs::read_dir(&self.project_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("csproj") {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        // Extract project name from filename (remove .csproj extension)
                        if let Some(file_stem) = path.file_stem() {
                            if let Some(name) = file_stem.to_str() {
                                return Some(name.to_string());
                            }
                        }

                        // Try to extract <AssemblyName> or <PackageId> from XML
                        for line in content.lines() {
                            let line = line.trim();
                            if line.starts_with("<AssemblyName>")
                                && line.ends_with("</AssemblyName>")
                            {
                                let name = line
                                    .trim_start_matches("<AssemblyName>")
                                    .trim_end_matches("</AssemblyName>");
                                if !name.is_empty() {
                                    return Some(name.to_string());
                                }
                            }
                            if line.starts_with("<PackageId>") && line.ends_with("</PackageId>") {
                                let name = line
                                    .trim_start_matches("<PackageId>")
                                    .trim_end_matches("</PackageId>");
                                if !name.is_empty() {
                                    return Some(name.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// Extract project name from pom.xml
    fn extract_from_pom_xml(&self) -> Option<String> {
        let pom_path = self.project_path.join("pom.xml");
        if !pom_path.exists() {
            return None;
        }

        match std::fs::read_to_string(&pom_path) {
            Ok(content) => {
                // Simple XML parsing, find <artifactId> or <name>
                let lines: Vec<&str> = content.lines().collect();
                for line in lines {
                    let line = line.trim();
                    // Prioritize <name> tag
                    if line.starts_with("<name>") && line.ends_with("</name>") {
                        let name = line
                            .trim_start_matches("<name>")
                            .trim_end_matches("</name>");
                        if !name.is_empty() {
                            return Some(name.to_string());
                        }
                    }
                    // Use <artifactId> tag as fallback
                    if line.starts_with("<artifactId>") && line.ends_with("</artifactId>") {
                        let name = line
                            .trim_start_matches("<artifactId>")
                            .trim_end_matches("</artifactId>");
                        if !name.is_empty() {
                            return Some(name.to_string());
                        }
                    }
                }
            }
            Err(_) => return None,
        }
        None
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            project_name: None,
            project_path: PathBuf::from("."),
            output_path: PathBuf::from("./tree.docs"),            
            internal_path: PathBuf::from("./.tree"),
            max_depth: 10,
            max_file_size: 64 * 1024, // 64KB
            include_tests: false,
            include_hidden: false,
            excluded_dirs: vec![
                ".tree".to_string(),
                "tree.docs".to_string(),
                "target".to_string(),
                "node_modules".to_string(),
                ".git".to_string(),
                "build".to_string(),
                "dist".to_string(),
                "venv".to_string(),
                ".svelte-kit".to_string(),
                "__pycache__".to_string(),
                "__tests__".to_string(),
                "__mocks__".to_string(),
                "__fixtures__".to_string(),
            ],
            excluded_files: vec![
                "tree.toml".to_string(),
                "*.tree".to_string(),
                "*.log".to_string(),
                "*.tmp".to_string(),
                "*.cache".to_string(),
                "bun.lock".to_string(),
                "package-lock.json".to_string(),
                "yarn.lock".to_string(),
                "pnpm-lock.yaml".to_string(),
                "Cargo.lock".to_string(),
                ".gitignore".to_string(),
                "*.tpl".to_string(),
                "*.md".to_string(),
                "*.txt".to_string(),
                ".env".to_string(),
            ],
            excluded_extensions: vec![
                "jpg".to_string(),
                "jpeg".to_string(),
                "png".to_string(),
                "gif".to_string(),
                "bmp".to_string(),
                "ico".to_string(),
                "mp3".to_string(),
                "mp4".to_string(),
                "avi".to_string(),
                "pdf".to_string(),
                "zip".to_string(),
                "tar".to_string(),
                "exe".to_string(),
                "dll".to_string(),
                "so".to_string(),
                "archive".to_string(),
            ],
            included_extensions: vec![],
            cache: CacheConfig::default(),
            verbose: false,
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            cache_dir: PathBuf::from(".tree"),
            expire_hours: 8760,
            max_parallels: 10,
        }
    }
}
