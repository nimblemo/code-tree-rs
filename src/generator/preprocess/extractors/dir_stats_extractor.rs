use anyhow::Result;
use serde::Serialize;
use std::path::PathBuf;

use super::{format_bytes, load_structure, normalize_path};
use crate::config::Config;

use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize)]
pub struct AdvancedMetrics {
    pub avg_function_length: f64,
    pub decision_density: f64,
    pub max_risk_score: f64,
    pub fan_in: usize,
    pub fan_out: usize,
    pub instability: f64,
    pub stack_distribution: HashMap<String, usize>,
    pub core_ratio: f64,
    pub avg_age_days: f64,
}

#[derive(Serialize)]
pub struct DirStats {
    pub path: String,
    pub file_count: usize,
    pub subdirectory_count: usize,
    pub total_size: u64,
    pub importance_score: f64,
    pub advanced_metrics: AdvancedMetrics,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub sizes: Option<Vec<u64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub complexity_scores: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lines_of_code: Option<Vec<usize>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub functions_counts: Option<Vec<usize>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cyclomatic_complexities: Option<Vec<f64>>,
}


pub async fn run_stats(config: &Config, input_path: &PathBuf, json: bool, dump: bool) -> Result<()> {
    let (structure, config, norm_rel, _cm) = load_structure(config, input_path, json).await?;

    let is_root = norm_rel.as_os_str().is_empty() || norm_rel == PathBuf::from(".");

    let stats = if is_root {
        // All files in the project
        let importance_scores: Vec<f64> = structure.files.iter().map(|f| f.importance_score).collect();
        let importance_score = if importance_scores.is_empty() {
            0.0
        } else {
            importance_scores.iter().sum::<f64>() / importance_scores.len() as f64
        };

        let descendants: Vec<&crate::types::FileInfo> = structure.files.iter().collect();
        let advanced_metrics = compute_advanced_metrics(&descendants, &structure.files);

        let (sizes, complexity_scores, lines_of_code, functions_counts, cyclomatic_complexities) = if dump {
            (
                Some(structure.files.iter().map(|f| f.size).collect()),
                Some(structure.files.iter().map(|f| f.complexity_score).collect()),
                Some(structure.files.iter().map(|f| f.lines_of_code).collect()),
                Some(structure.files.iter().map(|f| f.functions_count).collect()),
                Some(structure.files.iter().map(|f| f.cyclomatic_complexity).collect()),
            )
        } else {
            (None, None, None, None, None)
        };

        DirStats {
            path: structure.project_name.clone(),
            file_count: structure.total_files,
            subdirectory_count: structure.total_directories,
            total_size: structure.files.iter().map(|f| f.size).sum(),
            importance_score,
            advanced_metrics,
            sizes,
            complexity_scores,
            lines_of_code,
            functions_counts,
            cyclomatic_complexities,
        }
    } else {
        let target = structure.directories.iter().find(|d| {
            let rel = d.path.strip_prefix(&config.project_path).unwrap_or(&d.path);
            normalize_path(&rel.to_path_buf()) == norm_rel
        });

        match target {
            Some(d) => {
                // Collect ALL descendant files (recursive), not just direct children
                let descendants: Vec<&crate::types::FileInfo> = structure
                    .files
                    .iter()
                    .filter(|f| normalize_path(&f.path).starts_with(&norm_rel))
                    .collect();

                let advanced_metrics = compute_advanced_metrics(&descendants, &structure.files);

                let (sizes, complexity_scores, lines_of_code, functions_counts, cyclomatic_complexities) = if dump {
                    (
                        Some(descendants.iter().map(|f| f.size).collect()),
                        Some(descendants.iter().map(|f| f.complexity_score).collect()),
                        Some(descendants.iter().map(|f| f.lines_of_code).collect()),
                        Some(descendants.iter().map(|f| f.functions_count).collect()),
                        Some(descendants.iter().map(|f| f.cyclomatic_complexity).collect()),
                    )
                } else {
                    (None, None, None, None, None)
                };

                let file_count = descendants.len();
                let total_size = descendants.iter().map(|f| f.size).sum::<u64>();
                let subdirectory_count = structure.directories.iter()
                    .filter(|d_inner| {
                        let inner_rel = d_inner.path.strip_prefix(&config.project_path).unwrap_or(&d_inner.path);
                        let inner_norm = normalize_path(&inner_rel.to_path_buf());
                        inner_norm.starts_with(&norm_rel) && inner_norm != norm_rel
                    })
                    .count();

                DirStats {
                    path: norm_rel.display().to_string(),
                    file_count,
                    subdirectory_count,
                    total_size,
                    importance_score: d.importance_score,
                    advanced_metrics,
                    sizes,
                    complexity_scores,
                    lines_of_code,
                    functions_counts,
                    cyclomatic_complexities,
                }
            },
            None => {
                eprintln!(
                    "❌ '{}' not found.\n   Available directories:",
                    norm_rel.display()
                );
                for d in &structure.directories {
                    let rel = d.path.strip_prefix(&config.project_path).unwrap_or(&d.path);
                    eprintln!("     {}", rel.display());
                }
                return Ok(());
            }
        }
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&stats)?);
    } else {
        print_stats(&stats);
    }
    Ok(())
}

fn print_stats(s: &DirStats) {
    println!("\n📊 Stats: {}/", s.path);
    println!("   file_count:         {}", s.file_count);
    println!("   subdirectory_count: {}", s.subdirectory_count);
    println!("   total_size:         {}", format_bytes(s.total_size));
    println!("   importance_score:   {:.3}", s.importance_score);

    println!("\n🔍 Advanced Metrics:");
    println!("   Avg Function Length: {:.1} lines", s.advanced_metrics.avg_function_length);
    println!("   Decision Density:    {:.3} branches/loc", s.advanced_metrics.decision_density);
    println!("   Max Risk Score:      {:.3}", s.advanced_metrics.max_risk_score);
    println!("   Fan In:              {} (external files depend on this dir)", s.advanced_metrics.fan_in);
    println!("   Fan Out:             {} (dependencies outside this dir)", s.advanced_metrics.fan_out);
    println!("   Instability:         {:.2} (0=stable, 1=unstable)", s.advanced_metrics.instability);
    println!("   Core Ratio:          {:.1}%", s.advanced_metrics.core_ratio * 100.0);
    println!("   Avg Age:             {:.1} days", s.advanced_metrics.avg_age_days);
    
    let mut stack: Vec<_> = s.advanced_metrics.stack_distribution.iter().collect();
    stack.sort_by(|a, b| b.1.cmp(a.1));
    let stack_str: Vec<String> = stack.iter().map(|(ext, loc)| format!("{}: {} loc", ext, loc)).collect();
    println!("   stack_distribution:  [{}]\n", stack_str.join(", "));
}

pub fn compute_advanced_metrics(
    descendants: &[&crate::types::FileInfo],
    all_files: &[crate::types::FileInfo],
) -> AdvancedMetrics {
    let total_loc: usize = descendants.iter().map(|f| f.lines_of_code).sum();
    let total_functions: usize = descendants.iter().map(|f| f.functions_count).sum();
    let total_cyclo: f64 = descendants.iter().map(|f| f.cyclomatic_complexity).sum();
    
    let avg_function_length = if total_functions > 0 {
        total_loc as f64 / total_functions as f64
    } else {
        0.0
    };

    let decision_density = if total_loc > 0 {
        total_cyclo / total_loc as f64
    } else {
        0.0
    };

    let max_risk_score = descendants.iter().map(|f| {
        f.complexity_score * 0.5 + f.importance_score * 0.5
    }).fold(0.0f64, f64::max);

    let mut internal_names = HashSet::new();
    let mut internal_paths = HashSet::new();
    let mut internal_basenames = HashSet::new();
    for f in descendants {
        internal_names.insert(f.name.clone());
        internal_paths.insert(f.path.display().to_string().replace('\\', "/"));
        if let Some(stem) = f.path.file_stem() {
            internal_basenames.insert(stem.to_string_lossy().to_string());
        }
    }

    let mut fan_out = 0;
    for f in descendants {
        for dep in &f.dependencies {
            let dep_norm = dep.replace('\\', "/");
            let dep_stem = std::path::Path::new(dep).file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            
            if !internal_names.contains(&dep_norm) 
                && !internal_paths.contains(&dep_norm) 
                && !internal_basenames.contains(&dep_stem) 
            {
                fan_out += 1;
            }
        }
    }

    let mut fan_in = 0;
    for ext_file in all_files {
        let ext_path = ext_file.path.display().to_string().replace('\\', "/");
        if internal_paths.contains(&ext_path) {
            continue;
        }
        for dep in &ext_file.dependencies {
            let dep_norm = dep.replace('\\', "/");
            let dep_stem = std::path::Path::new(dep).file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();

            if internal_names.contains(&dep_norm) 
                || internal_paths.contains(&dep_norm) 
                || internal_basenames.contains(&dep_stem) 
            {
                fan_in += 1;
            }
        }
    }

    let instability = if fan_in + fan_out > 0 {
        fan_out as f64 / (fan_in as f64 + fan_out as f64)
    } else {
        0.0
    };

    let mut stack_distribution = HashMap::new();
    let mut core_loc = 0;
    
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let mut total_age_weighted_days = 0.0;

    for f in descendants {
        let ext = f.extension.clone().unwrap_or_else(|| "unknown".to_string());
        *stack_distribution.entry(ext).or_insert(0) += f.lines_of_code;

        if f.is_core {
            core_loc += f.lines_of_code;
        }

        if let Some(ts_str) = &f.last_modified {
            if let Ok(ts) = ts_str.parse::<u64>() {
                if now > ts {
                    let diff_secs = now - ts;
                    let age_days = diff_secs as f64 / 86400.0;
                    total_age_weighted_days += age_days * (f.lines_of_code as f64);
                }
            }
        }
    }

    let core_ratio = if total_loc > 0 {
        core_loc as f64 / total_loc as f64
    } else {
        0.0
    };

    let avg_age_days = if total_loc > 0 {
        total_age_weighted_days / total_loc as f64
    } else {
        0.0
    };

    AdvancedMetrics {
        avg_function_length,
        decision_density,
        max_risk_score,
        fan_in,
        fan_out,
        instability,
        stack_distribution,
        core_ratio,
        avg_age_days,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use crate::types::FileInfo;

    fn mock_file(
        path_str: &str,
        loc: usize,
        is_core: bool,
        funcs: usize,
        complexity: f64,
        deps: Vec<&str>,
    ) -> FileInfo {
        let path = PathBuf::from(path_str);
        FileInfo {
            name: path.file_name().unwrap().to_string_lossy().to_string(),
            path,
            size: 100,
            extension: PathBuf::from(path_str).extension().map(|s| s.to_string_lossy().to_string()),
            is_core,
            importance_score: 0.5,
            complexity_score: 0.5,
            lines_of_code: loc,
            functions_count: funcs,
            classes_count: 0,
            cyclomatic_complexity: complexity,
            dependencies: deps.into_iter().map(|s| s.to_string()).collect(),
            last_modified: Some("2024-01-01".to_string()),
        }
    }

    #[test]
    fn test_advanced_metrics_calculation() {
        let file1 = mock_file("src/core/logger.rs", 100, true, 4, 10.0, vec![]);
        let file2 = mock_file("src/app/main.rs", 300, false, 10, 50.0, vec!["../core/logger", "./utils/helper"]);
        let file3 = mock_file("src/app/utils/helper.rs", 100, false, 5, 20.0, vec!["../../core/logger"]);

        let all_files = vec![file1.clone(), file2.clone(), file3.clone()];
        
        let descendants = vec![&file2, &file3]; // Test stats for 'src/app/'

        let metrics = compute_advanced_metrics(&descendants, &all_files);

        // Core Ratio: 0 core files out of 2 descendants -> 0.0
        assert_eq!(metrics.core_ratio, 0.0);

        // Avg Func Length: (300 + 100) / (10 + 5) = 400 / 15 ≈ 26.66
        assert!((metrics.avg_function_length - 26.66).abs() < 0.1);

        // Decision Density: (50.0 + 20.0) / 400 = 70.0 / 400 = 0.175
        assert!((metrics.decision_density - 0.175).abs() < 0.01);

        // Fan-out: 
        // file2 depends on "../core/logger" (outside src/app/) -> yes.
        // file2 depends on "./utils/helper" (inside src/app/) -> no.
        // file3 depends on "../../core/logger" (outside src/app/) -> yes.
        // So 2 dependencies outside. But wait, Fan-Out counts the unique external components or files inside this directory that have dependencies outside. Let's see our implementation. fan_out counts files inside the directory that have AT LEAST ONE dependency outside. Since both file2 and file3 depend on logger (outside), they both trigger the counter. Result -> 2.
        assert_eq!(metrics.fan_out, 2);

        // Fan-in:
        // ext_file is file1. file1 depends on vec![]. fan_in = 0.
        assert_eq!(metrics.fan_in, 0);

        // Instability = fan_out / (fan_in + fan_out) = 2 / (0 + 2) = 1.0
        assert_eq!(metrics.instability, 1.0);

        // Test stats for 'src/core/'
        let descendants_core = vec![&file1];
        let metrics_core = compute_advanced_metrics(&descendants_core, &all_files);

        // Fan-in: external files `file2` and `file3` both depend on logger.
        // Both match `logger` stem. So fan_in must be 2.
        assert_eq!(metrics_core.fan_in, 2);
        
        // Fan-out: file1 has no deps.
        assert_eq!(metrics_core.fan_out, 0);

        // Instability: 0 / (2 + 0) = 0.0
        assert_eq!(metrics_core.instability, 0.0);
    }

    #[test]
    fn test_dependency_matching_stems() {
        // Our directory is src/ui/
        let ui_file = mock_file("src/ui/component.js", 100, false, 5, 10.0, vec!["../utils/logger", "./helper.js"]);
        let helper_file = mock_file("src/ui/helper.js", 50, false, 2, 5.0, vec![]);
        
        // External file
        let ext_file = mock_file("src/app.js", 200, true, 10, 20.0, vec!["./ui/component"]);

        let all_files = vec![ui_file.clone(), helper_file.clone(), ext_file.clone()];
        let descendants = vec![&ui_file, &helper_file];

        let metrics = compute_advanced_metrics(&descendants, &all_files);

        // Fan-out check:
        // component.js depends on "../utils/logger" -> External (logger not in descendants)
        // component.js depends on "./helper.js" -> Internal (helper.js is in descendants)
        // So 1 file in this dir (ui_file) has an external dependency.
        assert_eq!(metrics.fan_out, 1);

        // Fan-in check:
        // app.js depends on "./ui/component" -> Match "component" stem in descendants.
        // So 1 external file (app.js) depends on this directory.
        assert_eq!(metrics.fan_in, 1);

        // Instability: 1 / (1 + 1) = 0.5
        assert_eq!(metrics.instability, 0.5);
    }
}
