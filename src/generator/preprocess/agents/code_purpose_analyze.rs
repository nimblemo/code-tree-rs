use anyhow::Result;
use std::path::Path;

use crate::generator::context::GeneratorContext;
use crate::types::code::{CodePurpose, CodePurposeMapper};

/// Component type enhancer, combining rules and AI analysis
pub struct CodePurposeEnhancer;

impl CodePurposeEnhancer {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn execute(
        &self,
        _context: &GeneratorContext,
        file_path: &Path,
        file_name: &str,
        _file_content: &str,
    ) -> Result<CodePurpose> {
        // First use rule mapping
        let rule_based_type =
            CodePurposeMapper::map_by_path_and_name(&file_path.to_string_lossy(), file_name);
        Ok(rule_based_type)
    }
}