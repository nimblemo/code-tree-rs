use std::path::PathBuf;

use crate::{
    generator::preprocess::extractors::language_processors::LanguageProcessorManager,
};

pub fn read_code_source(
    language_processor: &LanguageProcessorManager,
    project_path: &PathBuf,
    file_path: &PathBuf,
) -> String {
    // Build full file path
    let full_path = project_path.join(file_path);

    // Read source code
    if let Ok(content) = std::fs::read_to_string(&full_path) {
        // If code is too long, intelligently truncate
        truncate_source_code(language_processor, &full_path, &content, 8_1024)
    } else {
        format!("Cannot read file: {}", full_path.display())
    }
}

fn truncate_source_code(
    language_processor: &LanguageProcessorManager,
    file_path: &std::path::Path,
    content: &str,
    max_length: usize,
) -> String {
    if content.len() <= max_length {
        return content.to_string();
    }

    // Smart truncation: prioritize function definitions, struct definitions, and other important parts
    let lines: Vec<&str> = content.lines().collect();
    let mut result = String::new();
    let mut current_length = 0;
    let mut important_lines = Vec::new();
    let mut other_lines = Vec::new();

    // Classify lines: important and regular
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if language_processor.is_important_line(file_path, trimmed) {
            important_lines.push((i, line));
        } else {
            other_lines.push((i, line));
        }
    }

    // First add important lines
    for (_, line) in &important_lines {
        if current_length + line.len() > max_length {
            break;
        }
        result.push_str(line);
        result.push('\n');
        current_length += line.len() + 1;
    }

    // Then add regular lines until length limit is reached
    for (_, line) in other_lines {
        if current_length + line.len() > max_length {
            break;
        }
        result.push_str(line);
        result.push('\n');
        current_length += line.len() + 1;
    }

    if current_length >= max_length {
        result.push_str(&format!("\n... (sampled lines from {} important lines) ...\n", important_lines.len()));
    }

    result
}
