pub mod code;
pub mod project_structure;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileInfo {
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
    pub extension: Option<String>,
    pub is_core: bool,
    pub importance_score: f64,
    pub complexity_score: f64,
    pub lines_of_code: usize,
    pub functions_count: usize,
    pub classes_count: usize,
    pub cyclomatic_complexity: f64,
    pub dependencies: Vec<String>,
    pub last_modified: Option<String>,
}

/// Directory information
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DirectoryInfo {
    pub path: PathBuf,
    pub name: String,
    pub file_count: usize,
    pub subdirectory_count: usize,
    pub total_size: u64,
    pub importance_score: f64,
    pub sizes: Vec<u64>,
    pub complexity_scores: Vec<f64>,
    pub lines_of_code: Vec<usize>,
    pub functions_counts: Vec<usize>,
    pub cyclomatic_complexities: Vec<f64>,
}
