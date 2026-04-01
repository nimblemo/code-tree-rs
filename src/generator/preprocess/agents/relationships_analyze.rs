use anyhow::Result;

use crate::{
    generator::context::GeneratorContext,
    types::{
        code::CodeInsight, code_releationship::RelationshipAnalysis,
        project_structure::ProjectStructure,
    },
};

pub struct RelationshipsAnalyze;

impl RelationshipsAnalyze {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn execute(
        &self,
        _context: &GeneratorContext,
        _code_insights: &Vec<CodeInsight>,
        _project_structure: &ProjectStructure,
    ) -> Result<RelationshipAnalysis> {
        // --- LLM Analysis Disabled ---
        // Instead of calling LLM, we just return an empty RelationshipAnalysis
        // This is purely mechanical parsing phase now.
        Ok(RelationshipAnalysis::default())
    }
}
