use anyhow::Result;
use serde::Serialize;
use std::path::PathBuf;

use super::{format_bytes, load_structure, normalize_path};
use crate::config::Config;

use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::cache::CacheManager;
use crate::types::code::{CodePurposeMapper, CodeInsight};
use crate::utils::file_utils::resolve_dependency_path;
use super::language_processors::LanguageProcessorManager;

#[derive(Serialize, Clone, Debug, Default)]
pub enum SizeTier {
    #[default]
    Nano,
    Micro,
    Macro,
    Enterprise,
}

impl SizeTier {
    pub fn from_metrics(files: usize, modules: usize, loc: usize, fan_out: usize) -> Self {
        if files > 500 && modules > 30 && loc > 100_000 && fan_out > 100 {
            SizeTier::Enterprise
        } else if files > 50 && modules > 5 && loc > 10_000 && fan_out > 20 {
            SizeTier::Macro
        } else if files > 10 && modules > 1 && loc > 1_000 {
            SizeTier::Micro
        } else {
            SizeTier::Nano
        }
    }
}

impl std::fmt::Display for SizeTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SizeTier::Nano => write!(f, "Nano"),
            SizeTier::Micro => write!(f, "Micro"),
            SizeTier::Macro => write!(f, "Macro"),
            SizeTier::Enterprise => write!(f, "Enterprise"),
        }
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct ModuleInfo {
    pub role: String,
    pub name: String,
    pub path: String,
    pub lines: usize,
    pub functions: usize,
    pub base_language: String,
    pub fan_in: usize,
    pub fan_out: usize,
}

#[derive(Serialize, Clone, Default)]
pub struct AdvancedMetrics {
    pub avg_function_length: f64,
    pub decision_density: f64,
    pub max_risk_score: f64,
    pub fan_in: usize,
    pub fan_out: usize,
    pub instability: f64,
    pub stack_distribution: HashMap<String, usize>,
    pub purpose_distribution: HashMap<String, usize>,
    pub core_ratio: f64,
    pub avg_age_days: f64,
}

#[derive(Serialize)]
pub struct DirStats {
    pub path: String,
    pub file_count: usize,
    pub subdirectory_count: usize,
    pub total_size: u64,
    pub total_lines: usize,
    pub total_functions: usize,
    pub total_cyclomatic_complexity: f64,
    pub max_complexity_score: f64,
    pub importance_score: f64,
    pub advanced_metrics: AdvancedMetrics,
    pub tier: SizeTier,
    pub base_language: String,
    pub identified_modules: Vec<ModuleInfo>,
    
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


fn get_base_language(stack_distribution: &HashMap<String, usize>) -> String {
    stack_distribution
        .iter()
        .max_by_key(|entry| entry.1)
        .map(|(ext, _)| ext.clone())
        .unwrap_or_else(|| "Unknown".to_string())
}

pub async fn identify_modules(
    structure: &crate::types::project_structure::ProjectStructure,
    project_path: &PathBuf,
    target_rel: &PathBuf,
    all_files: &[crate::types::FileInfo],
    lang_manager: &LanguageProcessorManager,
    cm: &Arc<RwLock<CacheManager>>,
) -> Vec<ModuleInfo> {
    let mut dir_metrics = HashMap::new();
    let effective_target = if target_rel.as_os_str().is_empty() || *target_rel == PathBuf::from(".") {
        PathBuf::new()
    } else {
        target_rel.clone()
    };
    
    // Calculate loc for all directories
    for dir in &structure.directories {
        let dir_rel = dir.path.strip_prefix(project_path).unwrap_or(&dir.path);
        let dir_norm = normalize_path(&dir_rel.to_path_buf());
        if !dir_norm.starts_with(&effective_target) {
            continue;
        }
        
        let descendants: Vec<&crate::types::FileInfo> = structure.files.iter()
            .filter(|f| {
                let f_rel = f.path.strip_prefix(project_path).unwrap_or(&f.path);
                normalize_path(&f_rel.to_path_buf()).starts_with(&dir_norm) && lang_manager.get_processor(&f.path).is_some()
            })
            .collect();
            
        let loc: usize = descendants.iter().map(|f| f.lines_of_code).sum();
        dir_metrics.insert(dir_norm, loc);
    }
    
    // Also insert the root (effective_target) itself if it's not in structure.directories
    if !dir_metrics.contains_key(&effective_target) {
        let root_descendants: Vec<&crate::types::FileInfo> = structure.files.iter()
            .filter(|f| {
                let f_rel = f.path.strip_prefix(project_path).unwrap_or(&f.path);
                normalize_path(&f_rel.to_path_buf()).starts_with(&effective_target) && lang_manager.get_processor(&f.path).is_some()
            })
            .collect();
        let root_loc: usize = root_descendants.iter().map(|f| f.lines_of_code).sum();
        dir_metrics.insert(effective_target.clone(), root_loc);
    }
    
    let mut modules = Vec::new();
    let mut to_process = vec![effective_target.clone()];
    
    while let Some(current_dir) = to_process.pop() {
        let current_loc = *dir_metrics.get(&current_dir).unwrap_or(&0);
        if current_loc == 0 {
            continue;
        }
        
        // Find direct children
        let mut direct_children_dirs = Vec::new();
        for dir in &structure.directories {
            let dir_rel = dir.path.strip_prefix(project_path).unwrap_or(&dir.path);
            let dir_norm = normalize_path(&dir_rel.to_path_buf());
            if dir_norm.starts_with(&current_dir) && dir_norm != current_dir {
                let rel = dir_norm.strip_prefix(&current_dir).unwrap();
                if rel.components().count() == 1 {
                    direct_children_dirs.push(dir_norm);
                }
            }
        }
        
        let descendants: Vec<&crate::types::FileInfo> = structure.files.iter()
            .filter(|f| {
                let f_rel = f.path.strip_prefix(project_path).unwrap_or(&f.path);
                normalize_path(&f_rel.to_path_buf()).starts_with(&current_dir) && lang_manager.get_processor(&f.path).is_some()
            })
            .collect();

        let direct_files: Vec<&crate::types::FileInfo> = descendants.iter()
            .filter(|f| {
                let f_rel = f.path.strip_prefix(project_path).unwrap_or(&f.path);
                let f_norm = normalize_path(&f_rel.to_path_buf());
                f_norm.parent().map(|p| normalize_path(&p.to_path_buf())) == Some(current_dir.clone())
            })
            .copied()
            .collect();

        let external_files: Vec<&crate::types::FileInfo> = all_files.iter()
            .filter(|f| {
                let f_rel = f.path.strip_prefix(project_path).unwrap_or(&f.path);
                !normalize_path(&f_rel.to_path_buf()).starts_with(&current_dir)
            })
            .collect();

        let mut best_score = 0;

        for f in &direct_files {
            let mut ext_incoming = 0;
            for ext in &external_files {
                let mut depends_on_f = false;
                for dep in &ext.dependencies {
                    if let Some(resolved) = resolve_dependency_path(&ext.path, dep, project_path) {
                        if resolved == f.path || resolved.with_extension("") == f.path.with_extension("") {
                            depends_on_f = true; 
                            break;
                        }
                        
                        // Support for index imports (e.g., importing a directory that resolves to index.js)
                        let resolved_index = resolved.join("index");
                        if resolved_index.with_extension("") == f.path.with_extension("") {
                            depends_on_f = true;
                            break;
                        }
                    }
                }
                if depends_on_f { ext_incoming += 1; }
            }

            let mut int_deps = 0;
            for dep in &f.dependencies {
                if let Some(resolved) = resolve_dependency_path(&f.path, dep, project_path) {
                    for desc in &descendants {
                        if desc.path == f.path { continue; }
                        if resolved == desc.path || resolved.with_extension("") == desc.path.with_extension("") {
                            int_deps += 1; 
                            break;
                        }
                        
                        let resolved_index = resolved.join("index");
                        if resolved_index.with_extension("") == desc.path.with_extension("") {
                            int_deps += 1;
                            break;
                        }
                    }
                }
            }

            let name_lower = f.name.to_lowercase();
            let is_entry = name_lower.starts_with("mod.") || name_lower.starts_with("index.") || name_lower.starts_with("main.") || name_lower.starts_with("lib.") || name_lower.starts_with("__init__");
            let name_bonus = if is_entry { 5 } else { 0 };

            let score = ext_incoming * 2 + int_deps + name_bonus;
            if score > best_score {
                best_score = score;
            }
        }

        let is_target_root = current_dir == effective_target;
        let has_subdirs = !direct_children_dirs.is_empty();

        let direct_loc: usize = direct_files.iter().map(|f| f.lines_of_code).sum();
        let direct_ratio = if current_loc > 0 {
            direct_loc as f64 / current_loc as f64
        } else {
            0.0
        };

        let file_ratio = if descendants.is_empty() {
            0.0
        } else {
            direct_files.len() as f64 / descendants.len() as f64
        };

        let is_container = if is_target_root && has_subdirs {
            true
        } else if direct_files.is_empty() {
            true
        } else if has_subdirs && (direct_ratio < 0.2 || file_ratio < 0.2) {
            true
        } else if best_score == 0 && has_subdirs {
            true
        } else {
            false // A directory is a module if it has any cohesive links or entry points
        };

        if is_container {
            to_process.extend(direct_children_dirs);
        } else {
            // It's a module
            if descendants.is_empty() {
                continue;
            }
                
            let metrics = compute_advanced_metrics(&descendants, all_files, cm, &structure.project_name).await;
            let name = current_dir.file_name().unwrap_or_default().to_string_lossy().to_string();
            let display_name = if name.is_empty() || current_dir == PathBuf::from(".") { "root".to_string() } else { name };
            
            let mut is_core = false;
            let mut is_interface = false;
            
            if metrics.fan_in > metrics.fan_out && metrics.instability < 0.3 {
                is_core = true;
            } else if metrics.fan_out > metrics.fan_in && metrics.instability > 0.7 {
                // Feature
            }
            
            let mut core_purposes = 0;
            let mut interface_purposes = 0;
            let mut total_purposes = 0;
            for (p, count) in &metrics.purpose_distribution {
                total_purposes += count;
                let p_lower = p.to_lowercase();
                if p_lower.contains("service") || p_lower.contains("model") || p_lower.contains("dao") || p_lower.contains("util") || p_lower.contains("data") {
                    core_purposes += count;
                } else if p_lower.contains("widget") || p_lower.contains("page") || p_lower.contains("api") || p_lower.contains("controller") || p_lower.contains("entry") {
                    interface_purposes += count;
                }
            }
            
            if total_purposes > 0 {
                if core_purposes as f64 / total_purposes as f64 > 0.5 {
                    is_core = true;
                }
                if interface_purposes as f64 / total_purposes as f64 > 0.5 {
                    is_interface = true;
                }
            }
            
            let role = if is_interface {
                "Interface".to_string()
            } else if is_core {
                "Core".to_string()
            } else {
                "Feature".to_string()
            };
            
            let path_str = if current_dir.as_os_str().is_empty() { ".".to_string() } else { current_dir.display().to_string() };
            
            modules.push(ModuleInfo {
                role,
                name: display_name,
                path: path_str,
                lines: current_loc,
                functions: descendants.iter().map(|f| f.functions_count).sum(),
                base_language: get_base_language(&metrics.stack_distribution),
                fan_in: metrics.fan_in,
                fan_out: metrics.fan_out,
            });
        }
    }
    
    modules.sort_by(|a, b| a.name.cmp(&b.name));
    modules
}

pub async fn run_stats(config: &Config, input_path: &PathBuf, json: bool, dump: bool) -> Result<()> {
    let (structure, config, norm_rel, _cm) = load_structure(config, input_path, json).await?;

    let is_root = norm_rel.as_os_str().is_empty() || norm_rel == PathBuf::from(".");
    let lang_manager = LanguageProcessorManager::new();

    let stats = if is_root {
        // All files in the project, filtered by code files only
        let descendants: Vec<&crate::types::FileInfo> = structure.files
            .iter()
            .filter(|f| lang_manager.get_processor(&f.path).is_some())
            .collect();
            
        let importance_scores: Vec<f64> = descendants.iter().map(|f| f.importance_score).collect();
        let importance_score = if importance_scores.is_empty() {
            0.0
        } else {
            importance_scores.iter().sum::<f64>() / importance_scores.len() as f64
        };

        let advanced_metrics = compute_advanced_metrics(&descendants, &structure.files, &_cm, &structure.project_name).await;
        
        let identified_modules = identify_modules(&structure, &config.project_path, &norm_rel, &structure.files, &lang_manager, &_cm).await;

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

        let total_lines = descendants.iter().map(|f| f.lines_of_code).sum();
        let total_functions = descendants.iter().map(|f| f.functions_count).sum();
        let total_cyclomatic_complexity = descendants.iter().map(|f| f.cyclomatic_complexity).sum();
        let max_complexity_score = descendants.iter().map(|f| f.complexity_score).fold(0.0_f64, |a, b| a.max(b));
        
        let file_count = descendants.len();
        let tier = SizeTier::from_metrics(file_count, identified_modules.len(), total_lines, advanced_metrics.fan_out);
        let base_language = get_base_language(&advanced_metrics.stack_distribution);

        DirStats {
            path: structure.project_name.clone(),
            file_count,
            subdirectory_count: structure.total_directories,
            total_size: descendants.iter().map(|f| f.size).sum(),
            total_lines,
            total_functions,
            total_cyclomatic_complexity,
            max_complexity_score,
            importance_score,
            advanced_metrics,
            tier,
            base_language,
            identified_modules,

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
                // Collect ALL descendant files (recursive), filtered by code files only
                let descendants: Vec<&crate::types::FileInfo> = structure
                    .files
                    .iter()
                    .filter(|f| normalize_path(&f.path).starts_with(&norm_rel) && lang_manager.get_processor(&f.path).is_some())
                    .collect();

                let advanced_metrics = compute_advanced_metrics(&descendants, &structure.files, &_cm, &structure.project_name).await;
                
                let identified_modules = identify_modules(&structure, &config.project_path, &norm_rel, &structure.files, &lang_manager, &_cm).await;

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
                let total_lines = descendants.iter().map(|f| f.lines_of_code).sum();
                let total_functions = descendants.iter().map(|f| f.functions_count).sum();
                let total_cyclomatic_complexity = descendants.iter().map(|f| f.cyclomatic_complexity).sum();
                let max_complexity_score = descendants.iter().map(|f| f.complexity_score).fold(0.0_f64, |a, b| a.max(b));
                let subdirectory_count = structure.directories.iter()
                    .filter(|d_inner| {
                        let inner_rel = d_inner.path.strip_prefix(&config.project_path).unwrap_or(&d_inner.path);
                        let inner_norm = normalize_path(&inner_rel.to_path_buf());
                        inner_norm.starts_with(&norm_rel) && inner_norm != norm_rel
                    })
                    .count();
                    
                let tier = SizeTier::from_metrics(file_count, identified_modules.len(), total_lines, advanced_metrics.fan_out);
                let base_language = get_base_language(&advanced_metrics.stack_distribution);

                DirStats {
                    path: norm_rel.display().to_string(),
                    file_count,
                    subdirectory_count,
                    total_size,
                    total_lines,
                    total_functions,
                    total_cyclomatic_complexity,
                    max_complexity_score,
                    importance_score: d.importance_score,
                    advanced_metrics,
                    tier,
                    base_language,
                    identified_modules,
                    
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
    println!("   tier:               {}", s.tier);
    println!("   base_language:      {}", s.base_language);
    println!("   file_count:         {}", s.file_count);
    println!("   subdirectory_count: {}", s.subdirectory_count);
    println!("   total_size:         {}", format_bytes(s.total_size));
    println!("   total_lines:        {}", s.total_lines);
    println!("   total_functions:    {}", s.total_functions);
    println!("   total_cyclo_complx: {:.3}", s.total_cyclomatic_complexity);
    println!("   max_complexity_scr: {:.3}", s.max_complexity_score);
    println!("   importance_score:   {:.3}", s.importance_score);

    println!("\nAdvanced Metrics:");
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
    println!("   stack_distribution:  [{}]", stack_str.join(", "));

    let mut purpose: Vec<_> = s.advanced_metrics.purpose_distribution.iter().collect();
    purpose.sort_by(|a, b| b.1.cmp(a.1));
    let purpose_str: Vec<String> = purpose.iter().map(|(p, loc)| format!("{}: {} loc", p, loc)).collect();
    println!("   purpose_distribution: [{}]\n", purpose_str.join(", "));

    if !s.identified_modules.is_empty() {
        println!("🔍 Identified Modules:");
        for m in &s.identified_modules {
            println!("   [{}] {} ({}, {} lines, {} functions) -> {}", m.role, m.name, m.base_language, m.lines, m.functions, m.path);
        }
        println!();
    }
}

pub async fn compute_advanced_metrics(
    descendants: &[&crate::types::FileInfo],
    all_files: &[crate::types::FileInfo],
    cm: &Arc<RwLock<CacheManager>>,
    project_name: &str,
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

    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut unique_fan_out = HashSet::new();
    for f in descendants {
        for dep in &f.dependencies {
            let dep_norm = dep.replace('\\', "/");
            let dep_stem = std::path::Path::new(dep).file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            
            let mut is_internal = false;
            if let Some(resolved) = resolve_dependency_path(&f.path, dep, std::path::Path::new("")) {
                let resolved_str = resolved.display().to_string().replace('\\', "/");
                let resolved_no_ext = resolved.with_extension("").display().to_string().replace('\\', "/");
                let resolved_index = resolved.join("index").with_extension("").display().to_string().replace('\\', "/");
                
                for ip in &internal_paths {
                    let ip_no_ext = std::path::Path::new(ip).with_extension("").display().to_string().replace('\\', "/");
                    if *ip == resolved_str || ip_no_ext == resolved_no_ext || ip_no_ext == resolved_index {
                        is_internal = true;
                        break;
                    }
                }
            }
            
            if !is_internal && (!internal_names.contains(&dep_norm) && !internal_paths.contains(&dep_norm) && !internal_basenames.contains(&dep_stem)) {
                let mut hasher = DefaultHasher::new();
                dep_norm.hash(&mut hasher);
                unique_fan_out.insert(hasher.finish());
            }
        }
    }
    let fan_out = unique_fan_out.len();

    let mut unique_fan_in = HashSet::new();
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

            let mut points_to_internal = false;
            if let Some(resolved) = resolve_dependency_path(&ext_file.path, dep, std::path::Path::new("")) {
                let resolved_str = resolved.display().to_string().replace('\\', "/");
                let resolved_no_ext = resolved.with_extension("").display().to_string().replace('\\', "/");
                let resolved_index = resolved.join("index").with_extension("").display().to_string().replace('\\', "/");
                
                for ip in &internal_paths {
                    let ip_no_ext = std::path::Path::new(ip).with_extension("").display().to_string().replace('\\', "/");
                    if *ip == resolved_str || ip_no_ext == resolved_no_ext || ip_no_ext == resolved_index {
                        points_to_internal = true;
                        break;
                    }
                }
            }

            if points_to_internal || internal_names.contains(&dep_norm) || internal_paths.contains(&dep_norm) || internal_basenames.contains(&dep_stem) {
                let mut hasher = DefaultHasher::new();
                ext_path.hash(&mut hasher);
                unique_fan_in.insert(hasher.finish());
            }
        }
    }
    let fan_in = unique_fan_in.len();

    let instability = if fan_in + fan_out > 0 {
        fan_out as f64 / (fan_in as f64 + fan_out as f64)
    } else {
        0.0
    };

    let mut stack_distribution = HashMap::new();
    let mut purpose_distribution = HashMap::new();
    let mut core_loc = 0;
    
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let mut total_age_weighted_days = 0.0;

    let cm_guard = cm.read().await;

    for f in descendants {
        let ext = f.extension.clone().unwrap_or_else(|| "unknown".to_string());
        *stack_distribution.entry(ext).or_insert(0) += f.lines_of_code;

        // CodePurpose determination
        let safe_path = f.path.display().to_string().replace(['\\', '/'], "_");
        let cache_key = format!("insight_{}_{}", project_name, safe_path);
        let purpose = if let Some(insight) = cm_guard.get::<CodeInsight>("insights", &cache_key).await {
            insight.code_dossier.code_purpose
        } else {
            CodePurposeMapper::map_by_path_and_name(&f.path.display().to_string(), &f.name)
        };
        let purpose_name = purpose.display_name().to_string();
        *purpose_distribution.entry(purpose_name).or_insert(0) += f.lines_of_code;

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
        purpose_distribution,
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

    #[tokio::test]
    async fn test_advanced_metrics_calculation() {
        let file1 = mock_file("src/core/logger.rs", 100, true, 4, 10.0, vec![]);
        let file2 = mock_file("src/app/main.rs", 300, false, 10, 50.0, vec!["../core/logger", "./utils/helper"]);
        let file3 = mock_file("src/app/utils/helper.rs", 100, false, 5, 20.0, vec!["../../core/logger"]);

        let all_files = vec![file1.clone(), file2.clone(), file3.clone()];
        
        let descendants = vec![&file2, &file3]; // Test stats for 'src/app/'

        let cm = Arc::new(RwLock::new(CacheManager::new(crate::config::CacheConfig::default())));
        let metrics = compute_advanced_metrics(&descendants, &all_files, &cm, "test_proj").await;

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
        let metrics_core = compute_advanced_metrics(&descendants_core, &all_files, &cm, "test_proj").await;

        // Fan-in: external files `file2` and `file3` both depend on logger.
        // Both match `logger` stem. So fan_in must be 2.
        assert_eq!(metrics_core.fan_in, 2);
        
        // Fan-out: file1 has no deps.
        assert_eq!(metrics_core.fan_out, 0);

        // Instability: 0 / (2 + 0) = 0.0
        assert_eq!(metrics_core.instability, 0.0);
    }

    #[tokio::test]
    async fn test_dependency_matching_stems() {
        // Our directory is src/ui/
        let ui_file = mock_file("src/ui/component.js", 100, false, 5, 10.0, vec!["../utils/logger", "./helper.js"]);
        let helper_file = mock_file("src/ui/helper.js", 50, false, 2, 5.0, vec![]);
        
        // External file
        let ext_file = mock_file("src/app.js", 200, true, 10, 20.0, vec!["./ui/component"]);

        let all_files = vec![ui_file.clone(), helper_file.clone(), ext_file.clone()];
        let descendants = vec![&ui_file, &helper_file];

        let cm = Arc::new(RwLock::new(CacheManager::new(crate::config::CacheConfig::default())));
        let metrics = compute_advanced_metrics(&descendants, &all_files, &cm, "test_proj").await;

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
