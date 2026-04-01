use crate::{
    generator::{
        context::GeneratorContext,
        preprocess::extractors::language_processors::LanguageProcessorManager,
    },
    types::{
        code::{CodeDossier, CodeInsight},
        project_structure::ProjectStructure,
    },
    utils::threads::do_parallel_with_limit,
};
use anyhow::Result;

pub struct CodeAnalyze {
    language_processor: LanguageProcessorManager,
}

impl CodeAnalyze {
    pub fn new() -> Self {
        Self {
            language_processor: LanguageProcessorManager::new(),
        }
    }

    pub async fn execute(
        &self,
        context: &GeneratorContext,
        codes: &Vec<CodeDossier>,
        project_structure: &ProjectStructure,
    ) -> Result<Vec<CodeInsight>> {
        let max_parallels = context.config.cache.max_parallels;

        // Create concurrent tasks
        let analysis_futures: Vec<_> = codes
            .iter()
            .map(|code| {
                let code_clone = code.clone();
                let project_structure_clone = project_structure.clone();
                let language_processor = self.language_processor.clone();

                Box::pin(async move {
                    let code_analyze = CodeAnalyze { language_processor };
                    let mut static_insight = code_analyze
                        .analyze_code_by_rules(&code_clone, &project_structure_clone)
                        .await?;
                    static_insight.code_dossier.source_summary = code_clone.source_summary.to_owned();

                    // --- LLM Analysis Disabled ---
                    // Directly use static_insight instead of calling LLM
                    Result::<CodeInsight>::Ok(static_insight)
                })
            })
            .collect();

        // Use do_parallel_with_limit for concurrency control
        let analysis_results = do_parallel_with_limit(analysis_futures, max_parallels).await;

        // Process analysis results
        let mut code_insights = Vec::new();
        for result in analysis_results {
            match result {
                Ok(code_insight) => {
                    code_insights.push(code_insight);
                }
                Err(e) => {
                    eprintln!("❌ Code analysis failed: {}", e);
                    return Err(e);
                }
            }
        }

        println!(
            "✓ Concurrent code analysis completed, successfully analyzed {} files",
            code_insights.len()
        );
        Ok(code_insights)
    }

    async fn analyze_code_by_rules(
        &self,
        code: &CodeDossier,
        project_structure: &ProjectStructure,
    ) -> Result<CodeInsight> {
        let full_path = project_structure.root_path.join(&code.file_path);

        // Read file content
        let content = if full_path.exists() {
            tokio::fs::read_to_string(&full_path).await?
        } else {
            String::new()
        };

        // Analyze interfaces
        let interfaces = self
            .language_processor
            .extract_interfaces(&code.file_path, &content);

        // Analyze dependencies
        let dependencies = self
            .language_processor
            .extract_dependencies(&code.file_path, &content);

        // Calculate complexity metrics
        let complexity_metrics = self
            .language_processor
            .calculate_complexity_metrics(&content);

        Ok(CodeInsight {
            code_dossier: code.clone(),
            interfaces,
            dependencies,
            complexity_metrics,
        })
    }
}
