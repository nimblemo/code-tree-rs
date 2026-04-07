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
pub struct Filter {
    pub metric: String,
    pub operator: Operator,
    pub value: f64,
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
            
            Some(Filter {
                value: if metric == "core" && value > 1.0 { value / 100.0 } else { value },
                metric,
                operator,
            })
        } else {
            None
        }
    }

    pub fn matches(&self, dir: &DirNode) -> bool {
        let actual_value = match self.metric.as_str() {
            "core" => dir.advanced_metrics.as_ref().map(|am| am.core_ratio).unwrap_or(0.0),
            "risk" => dir.advanced_metrics.as_ref().map(|am| am.max_risk_score).unwrap_or(0.0),
            "instability" => dir.advanced_metrics.as_ref().map(|am| am.instability).unwrap_or(0.0),
            "fan_in" => dir.advanced_metrics.as_ref().map(|am| am.fan_in as f64).unwrap_or(0.0),
            "fan_out" => dir.advanced_metrics.as_ref().map(|am| am.fan_out as f64).unwrap_or(0.0),
            "age" => dir.advanced_metrics.as_ref().map(|am| am.avg_age_days).unwrap_or(0.0),
            _ => return true, // Unknown metric, pass filter
        };

        match self.operator {
            Operator::GreaterThan => actual_value > self.value,
            Operator::LessThan => actual_value < self.value,
            Operator::GreaterOrEqual => actual_value >= self.value,
            Operator::LessOrEqual => actual_value <= self.value,
        }
    }
}

pub fn should_print_node(entry: &TreeEntry, filter: &Option<Filter>) -> bool {
    if let Some(f) = filter {
        match entry {
            TreeEntry::Dir(d) => should_print_dir(d, f),
            TreeEntry::File(_) => true, // Files are printed if their parent dir is printed
        }
    } else {
        true
    }
}

pub fn should_print_dir(d: &DirNode, f: &Filter) -> bool {
    if f.matches(d) {
        return true;
    }
    for child in &d.children {
        if let TreeEntry::Dir(child_dir) = child {
            if should_print_dir(child_dir, f) {
                return true;
            }
        }
    }
    false
}
