use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::time::Instant;

use crate::generator::preprocess::memory::{MemoryScope, ScopedKeys};
use crate::{
    generator::{
        context::GeneratorContext,
        preprocess::{
            agents::code_analyze::CodeAnalyze,
            extractors::structure_extractor::StructureExtractor,
        },
        types::Generator,
    },
    types::{
        code::CodeInsight,
        project_structure::ProjectStructure,
    },
};

pub mod agents;
pub mod extractors;
pub mod memory;

/// Preprocessing result
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PreprocessingResult {
    // Project structure information
    pub project_structure: ProjectStructure,
    // Intelligent insights of core code
    pub core_code_insights: Vec<CodeInsight>,
    pub processing_time: f64,
}

pub struct PreProcessAgent {}

impl PreProcessAgent {
    pub fn new() -> Self {
        Self {}
    }
}

impl Generator<PreprocessingResult> for PreProcessAgent {
    async fn execute(&self, context: GeneratorContext) -> Result<PreprocessingResult> {
        let start_time = Instant::now();

        let structure_extractor = StructureExtractor::new(context.clone());
        let config = &context.config;

        println!("🔍 Starting project preprocessing phase...");

        // 2. Extract project structure
        println!("📁 Extracting project structure...");
        let project_structure = structure_extractor
            .extract_structure(&config.project_path)
            .await?;

        println!(
            "   🔭 Discovered {} files, {} directories",
            project_structure.total_files, project_structure.total_directories
        );

        // 3. Identify core components
        println!("🎯 Identifying main source code files...");
        let important_codes = structure_extractor
            .identify_core_codes(&project_structure)
            .await?;

        println!("   Identified {} main source code files", important_codes.len());

       

        // 4. Analyze core components using AI
        println!("🤖 Analyzing core files using AI...");
        let code_analyze = CodeAnalyze::new();
        let core_code_insights = code_analyze
            .execute(&context, &important_codes, &project_structure)
            .await?;

        // Explicitly write the mechanical analysis results to cache
        // Instead of writing a single large array, write each file's insight separately
        for insight in &core_code_insights {
            // Generate a safe filename based on the file path (replace slashes)
            let safe_path_name = insight.code_dossier.file_path.display().to_string().replace("\\", "_").replace("/", "_");
            let cache_key = format!("insight_{}_{}", config.project_path.file_name().unwrap_or_default().to_string_lossy(), safe_path_name);
            
            context
                .cache_manager
                .write()
                .await
                .set("insights", &cache_key, insight)
                .await?;
        }
      
        // --- Back-fill section removed ---
        // (Metrics and complexity_score are now directly provided by structure_extractor
        // which gives us O(1) performance for `stats` command without reading CodeInsight).

        let processing_time = start_time.elapsed().as_secs_f64();
        println!("✅ Project preprocessing completed, took {:.2} seconds", processing_time);

        // 6. Store preprocessing results to Memory
        context
            .store_to_memory(
                MemoryScope::PREPROCESS,
                ScopedKeys::PROJECT_STRUCTURE,
                &project_structure,
            )
            .await?;
        context
            .store_to_memory(
                MemoryScope::PREPROCESS,
                ScopedKeys::CODE_INSIGHTS,
                &core_code_insights,
            )
            .await?;

        Ok(PreprocessingResult {
            project_structure,
            core_code_insights,
            processing_time,
        })
    }
}
