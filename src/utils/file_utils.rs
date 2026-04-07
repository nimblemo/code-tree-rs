use std::path::Path;

/// Check if a file is a test file
pub fn is_test_file(path: &Path) -> bool {
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();

    let path_str = path.to_string_lossy().to_lowercase();

    // Path-based checks (support different path separators)
    if path_str.contains("/test/")
        || path_str.contains("\\test\\")
        || path_str.contains("/tests/")
        || path_str.contains("\\tests\\")
        || path_str.contains("/__tests__/")
        || path_str.contains("\\__tests__\\")
        || path_str.contains("/spec/")
        || path_str.contains("\\spec\\")
        || path_str.contains("/specs/")
        || path_str.contains("\\specs\\")
        || path_str.starts_with("test/")
        || path_str.starts_with("test\\")
        || path_str.starts_with("tests/")
        || path_str.starts_with("tests\\")
        || path_str.starts_with("__tests__/")
        || path_str.starts_with("__tests__\\")
        || path_str.starts_with("spec/")
        || path_str.starts_with("spec\\")
        || path_str.starts_with("specs/")
        || path_str.starts_with("specs\\")
    {
        return true;
    }

    // Filename-based checks
    // Python test files
    if file_name.starts_with("test_") || file_name.ends_with("_test.py") {
        return true;
    }

    // JavaScript/TypeScript test files
    if file_name.ends_with(".test.js")
        || file_name.ends_with(".spec.js")
        || file_name.ends_with(".test.ts")
        || file_name.ends_with(".spec.ts")
        || file_name.ends_with(".test.jsx")
        || file_name.ends_with(".spec.jsx")
        || file_name.ends_with(".test.tsx")
        || file_name.ends_with(".spec.tsx")
    {
        return true;
    }

    // Java test files
    if file_name.ends_with("test.java") || file_name.ends_with("tests.java") {
        return true;
    }

    // C# test files
    if file_name.ends_with("test.cs") 
        || file_name.ends_with("tests.cs")
        || file_name.ends_with(".test.cs")
        || file_name.ends_with(".tests.cs") {
        return true;
    }

    // Rust test files
    if file_name.ends_with("_test.rs") || file_name.ends_with("_tests.rs") {
        return true;
    }

    // Go test files
    if file_name.ends_with("_test.go") {
        return true;
    }

    // C/C++ test files
    if file_name.ends_with("_test.c")
        || file_name.ends_with("_test.cpp")
        || file_name.ends_with("_test.cc")
        || file_name.ends_with("test.c")
        || file_name.ends_with("test.cpp")
        || file_name.ends_with("test.cc")
    {
        return true;
    }

    // Generic test filename patterns
    if file_name.contains("test")
        && (file_name.starts_with("test")
            || file_name.ends_with("test")
            || file_name.contains("_test_")
            || file_name.contains(".test.")
            || file_name.contains("-test-")
            || file_name.contains("-test.")
            || file_name.contains(".spec.")
            || file_name.contains("_spec_")
            || file_name.contains("-spec-")
            || file_name.contains("-spec."))
    {
        return true;
    }

    false
}

/// Check if a directory is a test directory
pub fn is_test_directory(dir_name: &str) -> bool {
    let name_lower = dir_name.to_lowercase();

    // Common test directory names
    matches!(
        name_lower.as_str(),
        "test"
            | "tests"
            | "__tests__"
            | "spec"
            | "specs"
            | "testing"
            | "test_data"
            | "testdata"
            | "fixtures"
            | "e2e"
            | "integration"
            | "unit"
            | "acceptance"
    ) || name_lower.ends_with("_test")
        || name_lower.ends_with("_tests")
        || name_lower.ends_with("-test")
        || name_lower.ends_with("-tests")
}

/// Check if a file path is a binary file
pub fn is_binary_file_path(path: &Path) -> bool {
    if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
        let ext_lower = extension.to_lowercase();
        matches!(
            ext_lower.as_str(),
            // Image files
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "ico" | "svg" | "webp" |
            // Audio files
            "mp3" | "wav" | "flac" | "aac" | "ogg" | "m4a" |
            // Video files
            "mp4" | "avi" | "mkv" | "mov" | "wmv" | "flv" | "webm" |
            // Compressed files
            "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" |
            // Executable files
            "exe" | "dll" | "so" | "dylib" | "bin" |
            // Document files
            "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" |
            // Font files
            "ttf" | "otf" | "woff" | "woff2" |
            // Other binary files
            "db" | "sqlite" | "sqlite3" | "dat" | "cache" |
            "archive"
        )
    } else {
        false
    }
}

/// Resolve a relative dependency path against a base file path
/// Returns a normalized relative path from the project root if successful
pub fn resolve_dependency_path(base_file: &Path, dep_str: &str, project_root: &Path) -> Option<std::path::PathBuf> {
    if dep_str.is_empty() {
        return None;
    }
    
    // If dependency is already an absolute path within project
    let dep_path = Path::new(dep_str);
    if dep_path.is_absolute() {
        if let Ok(rel) = dep_path.strip_prefix(project_root) {
            return Some(normalize_path_lexically(rel));
        }
        return None; // Absolute path outside project
    }

    // Resolve relative to the directory of the base file
    let base_dir = base_file.parent()?;
    let joined = base_dir.join(dep_path);
    
    // Clean path lexically (resolve . and ..)
    let cleaned = normalize_path_lexically(&joined);
    
    // If the path was already relative to project root (e.g. from structure.files)
    // or we successfully resolved it, ensure it uses standard separators
    Some(cleaned)
}

/// Lexically normalizes a path, resolving `.` and `..` without hitting the filesystem
pub fn normalize_path_lexically(path: &Path) -> std::path::PathBuf {
    use std::path::Component;
    let mut out = std::path::PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                // Pop the last component if it's a normal directory, 
                // but don't pop if it's empty or we're at the root
                if let Some(Component::Normal(_)) = out.components().last() {
                    out.pop();
                } else {
                    out.push(component);
                }
            }
            Component::CurDir => {}
            _ => out.push(component),
        }
    }
    
    // Convert backslashes to forward slashes for consistent internal representation
    let s = out.to_string_lossy().replace('\\', "/");
    std::path::PathBuf::from(s.trim_start_matches("./"))
}

