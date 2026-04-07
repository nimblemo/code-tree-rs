use anyhow::Result;
use serde::Serialize;
use std::path::PathBuf;

use super::{format_bytes, load_structure, normalize_path};
use super::dir_stats_extractor::{AdvancedMetrics, compute_advanced_metrics};
use crate::config::Config;
use crate::types::{DirectoryInfo, FileInfo};

// ─── Tree node types ─────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct FileNode {
    pub kind: &'static str,
    pub name: String,
    pub path: String,
    pub size: u64,
    pub lines_of_code: usize,
    pub complexity_score: f64,
    pub importance_score: f64,
}

#[derive(Serialize)]
pub struct DirNode {
    pub kind: &'static str,
    pub name: String,
    pub path: String,
    pub file_count: usize,
    pub subdirectory_count: usize,
    pub total_size: u64,
    pub importance_score: f64,
    pub advanced_metrics: Option<AdvancedMetrics>,
    pub children: Vec<TreeEntry>,

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

#[derive(Serialize)]
#[serde(untagged)]
pub enum TreeEntry {
    Dir(DirNode),
    File(FileNode),
}

// ─── Entry point ─────────────────────────────────────────────────────────────

pub async fn run_tree(config: &Config, input_path: &PathBuf, json: bool, dump: bool) -> Result<()> {
    let (structure, config, norm_rel, _cm) = load_structure(config, input_path, json).await?;

    let is_root = norm_rel.as_os_str().is_empty() || norm_rel == PathBuf::from(".");

    let mut metrics_by_dir = std::collections::HashMap::new();
    
    // Compute root metrics
    let root_descendants: Vec<&FileInfo> = structure.files.iter().collect();
    let root_am = compute_advanced_metrics(&root_descendants, &structure.files);
    metrics_by_dir.insert(PathBuf::new(), root_am.clone());
    metrics_by_dir.insert(PathBuf::from("."), root_am);
    
    // Compute metrics for all directories
    for dir in &structure.directories {
        let rel = dir.path.strip_prefix(&config.project_path).unwrap_or(&dir.path);
        let norm = normalize_path(&rel.to_path_buf());
        
        let descendants: Vec<&FileInfo> = structure.files.iter()
            .filter(|f| normalize_path(&f.path).starts_with(&norm))
            .collect();
            
        let am = compute_advanced_metrics(&descendants, &structure.files);
        metrics_by_dir.insert(norm, am);
    }

    let tree: DirNode = if is_root {
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

        let total_size: u64 = structure.files.iter().map(|f| f.size).sum();
        let importance_score = {
            let scores: Vec<f64> = structure.files.iter().map(|f| f.importance_score).collect();
            if scores.is_empty() { 0.0 } else { scores.iter().sum::<f64>() / scores.len() as f64 }
        };

        let _descendants: Vec<&FileInfo> = structure.files.iter().collect();
        let (children, advanced_metrics) = build_children(
            &PathBuf::new(),
            &structure.directories,
            &structure.files,
            &config.project_path,
            dump,
            &metrics_by_dir,
        );

        DirNode {
            kind: "dir",
            name: structure.project_name.clone(),
            path: ".".to_string(),
            file_count: structure.total_files,
            subdirectory_count: structure.total_directories,
            total_size,
            importance_score,
            advanced_metrics: Some(advanced_metrics),
            children,
            sizes,
            complexity_scores,
            lines_of_code,
            functions_counts,
            cyclomatic_complexities,
        }
    } else {
        match structure.directories.iter().find(|d| {
            let rel = d.path.strip_prefix(&config.project_path).unwrap_or(&d.path);
            normalize_path(&rel.to_path_buf()) == norm_rel
        }) {
            Some(d) => {
                let descendants: Vec<&FileInfo> = structure.files.iter()
                    .filter(|f| normalize_path(&f.path).starts_with(&norm_rel))
                    .collect();
                let (children, advanced_metrics) = build_children(
                    &norm_rel,
                    &structure.directories,
                    &structure.files,
                    &config.project_path,
                    dump,
                    &metrics_by_dir,
                );

                let (sizes, complexity_scores, lines_of_code, functions_counts, cyclomatic_complexities) = if dump {
                    (
                        Some(d.sizes.clone()),
                        Some(d.complexity_scores.clone()),
                        Some(d.lines_of_code.clone()),
                        Some(d.functions_counts.clone()),
                        Some(d.cyclomatic_complexities.clone()),
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

                DirNode {
                    kind: "dir",
                    name: d.name.clone(),
                    path: norm_rel.display().to_string(),
                    file_count,
                    subdirectory_count,
                    total_size,
                    importance_score: d.importance_score,
                    advanced_metrics: Some(advanced_metrics),
                    children,
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
        println!("{}", serde_json::to_string_pretty(&tree)?);
    } else {
        println!("\n{}/  [{} files, {}]", tree.name, tree.file_count, format_bytes(tree.total_size));
        
        if let Some(am) = &tree.advanced_metrics {
            print_advanced_metrics_node(am, "");
        }

        for (i, child) in tree.children.iter().enumerate() {
            print_entry(child, "", i == tree.children.len() - 1);
        }
        println!();
    }

    Ok(())
}

// ─── Tree builder ─────────────────────────────────────────────────────────────

fn build_children(
    parent_norm: &PathBuf,
    all_dirs: &[DirectoryInfo],
    all_files: &[FileInfo],
    project_path: &PathBuf,
    dump: bool,
    metrics_by_dir: &std::collections::HashMap<PathBuf, AdvancedMetrics>,
) -> (Vec<TreeEntry>, AdvancedMetrics) {
    let mut children: Vec<TreeEntry> = Vec::new();

    let am_acc = metrics_by_dir
        .get(parent_norm)
        .cloned()
        .unwrap_or_else(AdvancedMetrics::default);

    // Direct child directories
    for dir in all_dirs {
        let d_rel = normalize_path(
            &dir.path
                .strip_prefix(project_path)
                .unwrap_or(&dir.path)
                .to_path_buf(),
        );
        let is_child = d_rel
            .parent()
            .map(|p| normalize_path(&p.to_path_buf()) == *parent_norm)
            .unwrap_or(false);

        if !is_child {
            continue;
        }

        let (sub_children, sub_am) = build_children(&d_rel, all_dirs, all_files, project_path, dump, metrics_by_dir);

        let (sizes, complexity_scores, lines_of_code, functions_counts, cyclomatic_complexities) = if dump {
            (
                Some(dir.sizes.clone()),
                Some(dir.complexity_scores.clone()),
                Some(dir.lines_of_code.clone()),
                Some(dir.functions_counts.clone()),
                Some(dir.cyclomatic_complexities.clone()),
            )
        } else {
            (None, None, None, None, None)
        };

        let descendants: Vec<&FileInfo> = all_files.iter()
            .filter(|f| normalize_path(&f.path).starts_with(&d_rel))
            .collect();
        let file_count = descendants.len();
        let total_size = descendants.iter().map(|f| f.size).sum::<u64>();
        let subdirectory_count = all_dirs.iter()
            .filter(|d_inner| {
                let inner_rel = d_inner.path.strip_prefix(project_path).unwrap_or(&d_inner.path);
                let inner_norm = normalize_path(&inner_rel.to_path_buf());
                inner_norm.starts_with(&d_rel) && inner_norm != d_rel
            })
            .count();

        children.push(TreeEntry::Dir(DirNode {
            kind: "dir",
            name: dir.name.clone(),
            path: d_rel.display().to_string(),
            file_count,
            subdirectory_count,
            total_size,
            importance_score: dir.importance_score,
            advanced_metrics: Some(sub_am),
            children: sub_children,
            sizes,
            complexity_scores,
            lines_of_code,
            functions_counts,
            cyclomatic_complexities,
        }));
    }

    // Direct child files
    for file in all_files {
        let f_norm = normalize_path(&file.path);
        let is_child = f_norm
            .parent()
            .map(|p| normalize_path(&p.to_path_buf()) == *parent_norm)
            .unwrap_or(false);

        if !is_child {
            continue;
        }

        children.push(TreeEntry::File(FileNode {
            kind: "file",
            name: file.name.clone(),
            path: f_norm.display().to_string(),
            size: file.size,
            lines_of_code: file.lines_of_code,
            complexity_score: file.complexity_score,
            importance_score: file.importance_score,
        }));
    }

    (children, am_acc)
}

// ─── Console tree output ─────────────────────────────────────────────────────

fn print_advanced_metrics_node(am: &crate::generator::preprocess::extractors::dir_stats_extractor::AdvancedMetrics, prefix: &str) {
    if am.max_risk_score > 0.0 || am.fan_in > 0 || am.fan_out > 0 {
        println!("{}│   • 🎯 Risk: {:.2} | Fan In/Out: {}/{} | Instab: {:.2}", 
            prefix, am.max_risk_score, am.fan_in, am.fan_out, am.instability);
        println!("{}│   • ⚙️  Decision Density: {:.3} branches/loc | Avg Func: {:.1} lines", 
            prefix, am.decision_density, am.avg_function_length);
        println!("{}│   • 📦 Core: {:.1}% | Avg Age: {:.1} days", 
            prefix, am.core_ratio * 100.0, am.avg_age_days);
        
        let mut stack: Vec<_> = am.stack_distribution.iter().collect();
        stack.sort_by(|a, b| b.1.cmp(a.1));
        let stack_str: Vec<String> = stack.iter().map(|(ext, loc)| format!("{}: {} loc", ext, loc)).collect();
        if !stack_str.is_empty() {
            println!("{}│   • 🛠  Stack: [{}]", prefix, stack_str.join(", "));
        }
    }
}

fn print_entry(entry: &TreeEntry, prefix: &str, is_last: bool) {
    let connector = if is_last { "└── " } else { "├── " };
    let extension = if is_last { "    " } else { "│   " };

    match entry {
        TreeEntry::Dir(d) => {
            println!(
                "{}{}📁 {}/  [{} files, {}]",
                prefix,
                connector,
                d.name,
                d.file_count,
                format_bytes(d.total_size)
            );
            
            let new_prefix = format!("{}{}", prefix, extension);
            
            if let Some(am) = &d.advanced_metrics {
                print_advanced_metrics_node(am, &new_prefix);
            }

            for (i, child) in d.children.iter().enumerate() {
                print_entry(child, &new_prefix, i == d.children.len() - 1);
            }
        }
        TreeEntry::File(f) => {
            println!(
                "{}{}📄 {}  ({}, loc: {}, c: {:.2})",
                prefix,
                connector,
                f.name,
                format_bytes(f.size),
                f.lines_of_code,
                f.complexity_score
            );
        }
    }
}
