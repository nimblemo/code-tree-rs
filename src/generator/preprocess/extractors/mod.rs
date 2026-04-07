pub mod dir_stats_extractor;
pub mod dir_tree_extractor;
pub mod language_processors;
pub mod structure_extractor;
pub mod filter;

use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::{
    cache::CacheManager,
    config::Config,
    generator::{
        context::GeneratorContext,
        preprocess::extractors::structure_extractor::StructureExtractor,
    },
    memory::Memory,
    types::project_structure::ProjectStructure,
};

pub fn normalize_path(p: &PathBuf) -> PathBuf {
    let s = p.to_string_lossy().replace('\\', "/");
    let s = s.trim_start_matches("./");
    PathBuf::from(s)
}

pub fn find_project_root(path: &PathBuf) -> Option<PathBuf> {
    let mut current = path.as_path();
    loop {
        if current.join(".tree").exists() {
            return Some(current.to_path_buf());
        }
        current = current.parent()?;
    }
}

/// Resolve path, load (or re-scan) ProjectStructure from cache.
/// Returns (structure, updated_config, norm_rel, cache_manager).
pub async fn load_structure(
    config: &Config,
    input_path: &PathBuf,
    quiet: bool,
) -> Result<(ProjectStructure, Config, PathBuf, Arc<RwLock<CacheManager>>)> {
    let mut config = config.clone();

    let effective_rel: PathBuf = if input_path.is_absolute() {
        // Normalize: collect components removes trailing separators (e.g. "path\")
        let input_path: PathBuf = input_path.components().collect();

        if let Ok(stripped) = input_path.strip_prefix(&config.project_path) {
            stripped.to_path_buf()
        } else {
            let project_root = find_project_root(&input_path).unwrap_or_else(|| {
                input_path.parent().unwrap_or(input_path.as_path()).to_path_buf()
            });
            if !quiet {
                println!("📍 Detected project root: {}", project_root.display());
            }
            let rel = input_path
                .strip_prefix(&project_root)
                .map(|p| p.to_path_buf())
                .unwrap_or_default();
            config.cache.cache_dir = project_root.join(".tree");
            config.project_path = project_root;
            rel
        }
    } else {
        input_path.clone()
    };

    let cache_manager = Arc::new(RwLock::new(CacheManager::new(config.cache.clone())));
    let memory = Arc::new(RwLock::new(Memory::new()));
    let context = GeneratorContext {
        config: config.clone(),
        cache_manager: cache_manager.clone(),
        memory,
    };

    let cache_key = format!("structure_{}", config.project_path.display());

    let cached = {
        let cm = cache_manager.read().await;
        cm.get::<ProjectStructure>("structure", &cache_key).await
    };

    let structure = match cached {
        Some(s) if s.total_files > 0 => {
            if !quiet {
                println!("✅ Loaded from cache ({} files, {} dirs)", s.total_files, s.total_directories);
            }
            s
        }
        _ => {
            if !quiet {
                println!("⚡ Cache miss — scanning...");
            }
            let extractor = StructureExtractor::new(context);
            let s = extractor.extract_structure(&config.project_path).await?;
            if !quiet {
                println!("   Scanned {} files, {} dirs", s.total_files, s.total_directories);
            }
            s
        }
    };

    // Re-persist if old cache lacks sizes/complexity_scores
    let needs_resave = structure.directories.iter().any(|d| d.sizes.is_empty() && d.file_count > 0);
    if needs_resave {
        if !quiet { println!("🔄 Updating cache..."); }
        cache_manager.write().await.set("structure", &cache_key, &structure).await?;
    }

    let norm_rel = normalize_path(&effective_rel);
    Ok((structure, config, norm_rel, cache_manager))
}

pub fn format_bytes(bytes: u64) -> String {
    match bytes {
        b if b < 1024 => format!("{} B", b),
        b if b < 1024 * 1024 => format!("{:.1} KB", b as f64 / 1024.0),
        b => format!("{:.1} MB", b as f64 / (1024.0 * 1024.0)),
    }
}
