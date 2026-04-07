use super::dir_tree_extractor::{TreeEntry, DirNode};
use std::sync::LazyLock;
use regex::Regex;

#[derive(Debug, Clone, Copy)]
pub enum Operator {
    GreaterThan,
    LessThan,
    GreaterOrEqual,
    LessOrEqual,
}

#[derive(Debug, Clone)]
pub enum Filter {
    Metric {
        metric: String,
        operator: Operator,
        value: f64,
    },
    Extension(Vec<String>),
}

static RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*([a-zA-Z_]+)\s*(>=|<=|>|<|=)\s*([0-9]+(?:\.[0-9]+)?)\s*$").unwrap()
});

impl Filter {
    pub fn parse(input: &str) -> Option<Self> {
        // Strip square brackets if the user used --filter[core=80]
        let input = input.trim_start_matches('[').trim_end_matches(']');
        
        if let Some(caps) = RE.captures(input) {
            let metric = caps.get(1)?.as_str().to_string();
            let op_str = caps.get(2)?.as_str();
            let value_str = caps.get(3)?.as_str();
            
            let value: f64 = value_str.parse().ok()?;
            
            let operator = match op_str {
                ">=" => Operator::GreaterOrEqual,
                "<=" => Operator::LessOrEqual,
                ">" => Operator::GreaterThan,
                "<" => Operator::LessThan,
                "=" => Operator::GreaterOrEqual, // According to plan, "=" is treated as ">="
                _ => return None,
            };
            
            Some(Filter::Metric {
                value: if metric == "core" && value > 1.0 { value / 100.0 } else { value },
                metric,
                operator,
            })
        } else {
            // Check if it's a comma-separated list of extensions
            let exts: Vec<String> = input.split(',')
                .map(|s| s.trim().trim_start_matches('.').to_lowercase())
                .filter(|s| !s.is_empty())
                .collect();
                
            if !exts.is_empty() && exts.iter().all(|ext| ext.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')) {
                Some(Filter::Extension(exts))
            } else {
                None
            }
        }
    }

    pub fn matches_dir(&self, dir: &DirNode) -> bool {
        match self {
            Filter::Metric { metric, operator, value } => {
                let actual_value = match metric.as_str() {
                    "core" => dir.advanced_metrics.as_ref().map(|am| am.core_ratio).unwrap_or(0.0),
                    "risk" => dir.advanced_metrics.as_ref().map(|am| am.max_risk_score).unwrap_or(0.0),
                    "instability" => dir.advanced_metrics.as_ref().map(|am| am.instability).unwrap_or(0.0),
                    "fan_in" => dir.advanced_metrics.as_ref().map(|am| am.fan_in as f64).unwrap_or(0.0),
                    "fan_out" => dir.advanced_metrics.as_ref().map(|am| am.fan_out as f64).unwrap_or(0.0),
                    "age" => dir.advanced_metrics.as_ref().map(|am| am.avg_age_days).unwrap_or(0.0),
                    _ => return true, // Unknown metric, pass filter
                };

                match operator {
                    Operator::GreaterThan => actual_value > *value,
                    Operator::LessThan => actual_value < *value,
                    Operator::GreaterOrEqual => actual_value >= *value,
                    Operator::LessOrEqual => actual_value <= *value,
                }
            },
            Filter::Extension(exts) => {
                // A directory matches if any of its files match the extension
                if let Some(am) = &dir.advanced_metrics {
                    exts.iter().any(|ext| am.stack_distribution.contains_key(ext))
                } else {
                    false
                }
            }
        }
    }
}

pub fn should_print_node(entry: &TreeEntry, filter: &Option<Filter>) -> bool {
    if let Some(f) = filter {
        match entry {
            TreeEntry::Dir(d) => should_print_dir(d, f),
            TreeEntry::File(file) => {
                if let Filter::Extension(exts) = f {
                    // Get file extension from its name
                    let parts: Vec<&str> = file.name.split('.').collect();
                    if parts.len() > 1 {
                        let ext = parts.last().unwrap().to_lowercase();
                        exts.contains(&ext)
                    } else {
                        false
                    }
                } else {
                    true // For metric filters, files are printed if their parent dir is printed
                }
            }
        }
    } else {
        true
    }
}

pub fn should_print_dir(d: &DirNode, f: &Filter) -> bool {
    if f.matches_dir(d) {
        return true;
    }
    for child in &d.children {
        match child {
            TreeEntry::Dir(child_dir) => {
                if should_print_dir(child_dir, f) {
                    return true;
                }
            },
            TreeEntry::File(file) => {
                if let Filter::Extension(exts) = f {
                    let parts: Vec<&str> = file.name.split('.').collect();
                    if parts.len() > 1 {
                        let ext = parts.last().unwrap().to_lowercase();
                        if exts.contains(&ext) {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}
