use std::path::Path;

use crate::types::code::{CodeComplexity, Dependency, InterfaceInfo};

/// Language processor trait
pub trait LanguageProcessor: Send + Sync + std::fmt::Debug {
    /// Get supported file extensions
    fn supported_extensions(&self) -> Vec<&'static str>;

    /// Extract file dependencies
    fn extract_dependencies(&self, content: &str, file_path: &Path) -> Vec<Dependency>;

    /// Determine component type
    #[allow(dead_code)]
    fn determine_component_type(&self, file_path: &Path, content: &str) -> String;

    /// Identify important code lines
    fn is_important_line(&self, line: &str) -> bool;

    /// Get language name
    #[allow(dead_code)]
    fn language_name(&self) -> &'static str;

    /// Extract code interface definitions
    fn extract_interfaces(&self, content: &str, file_path: &Path) -> Vec<InterfaceInfo>;

    /// Language-specific branching keywords for cyclomatic proxy.
    /// Override this to provide accurate per-language keyword lists.
    fn branch_keywords(&self) -> &'static [&'static str] {
        &[" if ", " else", " for ", " while ", "&&", "||", " switch", " case "]
    }

    /// Max nesting depth computation. Defaults to brace-counting.
    /// Override for indent-based languages (Python, etc.).
    fn compute_nesting_depth(&self, content: &str) -> usize {
        let mut depth = 0usize;
        let mut max_depth = 0usize;
        for ch in content.chars() {
            match ch {
                '{' => { depth += 1; max_depth = max_depth.max(depth); }
                '}' => { depth = depth.saturating_sub(1); }
                _ => {}
            }
        }
        max_depth
    }

    /// Compute a normalised complexity score in [0.0, 1.0].
    ///
    /// For **core files** this is later replaced by a more accurate score derived
    /// from `CodeInsight.complexity_metrics` in the preprocessing back-fill.
    /// For all other files this is the authoritative score.
    ///
    /// Uses three components weighted as: cyclomatic 50%, nesting 30%, function density 20%.
    fn compute_complexity_score(&self, content: &str) -> f64 {
        if content.is_empty() { return 0.0; }

        let loc = content.lines()
            .filter(|l| !l.trim().is_empty())
            .count();
        if loc == 0 { return 0.0; }

        // Cyclomatic proxy: language-specific keywords
        let decisions: usize = self.branch_keywords()
            .iter()
            .map(|kw| content.matches(kw).count())
            .sum();

        let max_depth = self.compute_nesting_depth(content);

        let func_kws = ["fn ", "function ", "def ", "func ", "fun "];
        let functions: usize = func_kws.iter().map(|kw| content.matches(kw).count()).sum();

        let cyclo   = (decisions as f64 / loc as f64 / 0.3).min(1.0);
        let nesting = (max_depth as f64 / 8.0).min(1.0);
        let func_d  = (functions as f64 / (loc as f64 / 10.0).max(1.0)).min(1.0);

        (cyclo * 0.50 + nesting * 0.30 + func_d * 0.20).min(1.0)
    }
}

/// Language processor manager
#[derive(Debug)]
pub struct LanguageProcessorManager {
    processors: Vec<Box<dyn LanguageProcessor>>,
}

impl Clone for LanguageProcessorManager {
    fn clone(&self) -> Self {
        Self::new()
    }
}

impl LanguageProcessorManager {
    pub fn new() -> Self {
        Self {
            processors: vec![
                Box::new(rust::RustProcessor::new()),
                Box::new(javascript::JavaScriptProcessor::new()),
                Box::new(typescript::TypeScriptProcessor::new()),
                Box::new(php::PhpProcessor::new()),
                Box::new(react::ReactProcessor::new()),
                Box::new(vue::VueProcessor::new()),
                Box::new(svelte::SvelteProcessor::new()),
                Box::new(kotlin::KotlinProcessor::new()),
                Box::new(python::PythonProcessor::new()),
                Box::new(java::JavaProcessor::new()),
                Box::new(csharp::CSharpProcessor::new()),
                Box::new(swift::SwiftProcessor::new()),
            ],
        }
    }

    /// Get processor by file extension
    pub fn get_processor(&self, file_path: &Path) -> Option<&dyn LanguageProcessor> {
        let extension = file_path.extension()?.to_str()?;

        for processor in &self.processors {
            if processor.supported_extensions().contains(&extension) {
                return Some(processor.as_ref());
            }
        }

        None
    }

    /// Extract file dependencies
    pub fn extract_dependencies(&self, file_path: &Path, content: &str) -> Vec<Dependency> {
        if let Some(processor) = self.get_processor(file_path) {
            processor.extract_dependencies(content, file_path)
        } else {
            Vec::new()
        }
    }

    /// Determine component type
    #[allow(dead_code)]
    pub fn determine_component_type(&self, file_path: &Path, content: &str) -> String {
        if let Some(processor) = self.get_processor(file_path) {
            processor.determine_component_type(file_path, content)
        } else {
            "unknown".to_string()
        }
    }

    /// Identify important code lines
    pub fn is_important_line(&self, file_path: &Path, line: &str) -> bool {
        if let Some(processor) = self.get_processor(file_path) {
            processor.is_important_line(line)
        } else {
            false
        }
    }

    /// Compute a normalised complexity score [0.0, 1.0] for a file.
    /// Uses the language-specific processor if available, else the default heuristic.
    pub fn compute_complexity_score(&self, file_path: &Path, content: &str) -> f64 {
        if let Some(processor) = self.get_processor(file_path) {
            processor.compute_complexity_score(content)
        } else {
            let loc = content.lines().filter(|l| !l.trim().is_empty()).count();
            if loc == 0 {
                return 0.0;
            }
            let decisions: usize = [" if ", " else", " for ", " while ", "&&", "||"]
                .iter()
                .map(|kw| content.matches(kw).count())
                .sum();
            (decisions as f64 / loc as f64 / 0.3).min(1.0)
        }
    }

    /// Extract code interface definitions
    pub fn extract_interfaces(&self, file_path: &Path, content: &str) -> Vec<InterfaceInfo> {
        if let Some(processor) = self.get_processor(file_path) {
            processor.extract_interfaces(content, file_path)
        } else {
            Vec::new()
        }
    }

    pub fn calculate_complexity_metrics(&self, content: &str) -> CodeComplexity {
        let lines: Vec<&str> = content.lines().collect();
        let lines_of_code = lines.len();

        // Simplified complexity calculation
        let number_of_functions = content.matches("fn ").count()
            + content.matches("def ").count()
            + content.matches("function ").count()
            + content.matches("async ").count();  // C# async methods

        let number_of_classes =
            content.matches("class ").count() 
            + content.matches("struct ").count()
            + content.matches("interface ").count();  // C# interfaces

        // Simplified cyclomatic complexity calculation
        let cyclomatic_complexity = 1.0
            + content.matches("if ").count() as f64
            + content.matches("while ").count() as f64
            + content.matches("for ").count() as f64
            + content.matches("foreach ").count() as f64  // C# foreach
            + content.matches("match ").count() as f64
            + content.matches("switch ").count() as f64  // C# switch
            + content.matches("case ").count() as f64;

        CodeComplexity {
            cyclomatic_complexity,
            lines_of_code,
            number_of_functions,
            number_of_classes,
        }
    }
}

// Submodules
pub mod csharp;
pub mod java;
pub mod javascript;
pub mod kotlin;
pub mod php;
pub mod python;
pub mod react;
pub mod rust;
pub mod svelte;
pub mod swift;
pub mod typescript;
pub mod vue;
