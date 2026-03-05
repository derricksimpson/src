use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use rayon::prelude::*;

use crate::exclusion::ExclusionFilter;
use crate::glob;
use crate::models::ScanResult;

const SOURCE_EXTENSIONS: &[&str] = &[
    "cs", "ts", "tsx", "js", "jsx", "py", "rb", "go", "rs",
    "java", "kt", "scala", "swift", "m", "mm", "c", "cpp",
    "cc", "cxx", "h", "hpp", "hxx", "lua", "pl", "pm",
    "php", "r", "dart", "ex", "exs", "erl", "hs", "fs",
    "fsx", "fsi", "ml", "mli", "v", "sv", "vhd", "vhdl",
    "sql", "sh", "bash", "zsh", "ps1", "psm1", "bat", "cmd",
    "yaml", "yml", "json", "xml", "html", "htm", "css",
    "scss", "sass", "less", "vue", "svelte", "astro",
    "md", "mdx", "rst", "txt", "toml", "ini", "cfg",
    "conf", "env", "dockerfile", "tf", "tfvars", "hcl",
    "proto", "graphql", "gql", "razor", "cshtml", "csproj",
    "sln", "gradle", "cmake", "makefile", "mk",
];

fn is_source_file(name: &str) -> bool {
    if let Some(dot_pos) = name.rfind('.') {
        let ext = &name[dot_pos + 1..];
        SOURCE_EXTENSIONS.iter().any(|&e| ext.eq_ignore_ascii_case(e))
    } else {
        let lower = name.to_ascii_lowercase();
        lower == "makefile" || lower == "dockerfile" || lower == "rakefile" || lower == "gemfile"
    }
}

pub fn scan_directories(
    root: &Path,
    filter: &ExclusionFilter,
    cancelled: &AtomicBool,
    include_tests: bool,
) -> ScanResult {
    if cancelled.load(Ordering::Relaxed) {
        return ScanResult { name: String::new(), children: None, files: None };
    }

    let dir_name = root.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.to_string_lossy().into_owned());

    let mut files = Vec::new();
    let mut subdirs = Vec::new();

    let entries = match fs::read_dir(root) {
        Ok(rd) => rd,
        Err(_) => return ScanResult { name: dir_name, children: None, files: None },
    };

    for entry in entries.flatten() {
        let ft = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if ft.is_file() {
            if is_source_file(&name_str) {
                if include_tests || !is_test_file(&name_str) {
                    files.push(name_str.into_owned());
                }
            }
        } else if ft.is_dir() && !filter.is_excluded(&name_str) {
            if include_tests || !matches!(name_str.to_ascii_lowercase().as_str(),
                "tests" | "test" | "__tests__" | "spec" | "specs") {
                subdirs.push(entry.path());
            }
        }
    }

    let children: Vec<ScanResult> = subdirs
        .par_iter()
        .filter_map(|subdir| {
            if cancelled.load(Ordering::Relaxed) {
                return None;
            }
            let child = scan_directories(subdir, filter, cancelled, include_tests);
            if child.files.is_some() || child.children.is_some() {
                Some(child)
            } else {
                None
            }
        })
        .collect();

    let mut children = children;
    children.sort_by(|a, b| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()));
    files.sort_by(|a: &String, b: &String| a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase()));

    ScanResult {
        name: dir_name,
        children: if children.is_empty() { None } else { Some(children) },
        files: if files.is_empty() { None } else { Some(files) },
    }
}

pub fn is_test_file(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    let lower = normalized.to_ascii_lowercase();

    for component in lower.split('/') {
        if matches!(component, "tests" | "test" | "__tests__" | "spec" | "specs") {
            return true;
        }
    }

    let filename = lower.rsplit('/').next().unwrap_or(&lower);
    if let Some(dot_pos) = filename.find('.') {
        let stem = &filename[..dot_pos];
        let after_first_dot = &filename[dot_pos..];

        if stem.starts_with("test_") {
            return true;
        }
        if stem.ends_with("_test") || stem.ends_with("_spec") {
            return true;
        }
        if after_first_dot.starts_with(".test.") || after_first_dot.starts_with(".spec.") {
            return true;
        }
        let without_first_ext = &stem;
        if let Some(inner_dot) = without_first_ext.rfind('.') {
            let inner_suffix = &without_first_ext[inner_dot..];
            if inner_suffix == ".test" || inner_suffix == ".spec" {
                return true;
            }
        }
    }

    false
}

pub fn find_files_filtered(
    root: &Path,
    globs: &[String],
    filter: &ExclusionFilter,
    cancelled: &AtomicBool,
    include_tests: bool,
) -> Vec<String> {
    let files = find_files(root, globs, filter, cancelled);
    if include_tests {
        files
    } else {
        files.into_iter().filter(|f| {
            let rel = crate::path_helper::normalized_relative(root, std::path::Path::new(f));
            !is_test_file(&rel)
        }).collect()
    }
}

pub fn find_files(
    root: &Path,
    globs: &[String],
    filter: &ExclusionFilter,
    cancelled: &AtomicBool,
) -> Vec<String> {
    let has_path_glob = globs.iter().any(|g| g.contains('/') || g.contains('\\'));
    let mut results = find_files_parallel(root, root, globs, has_path_glob, filter, cancelled);
    results.sort_by(|a, b| a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase()));
    results
}

fn find_files_parallel(
    root: &Path,
    dir: &Path,
    globs: &[String],
    has_path_glob: bool,
    filter: &ExclusionFilter,
    cancelled: &AtomicBool,
) -> Vec<String> {
    if cancelled.load(Ordering::Relaxed) {
        return Vec::new();
    }

    let entries = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return Vec::new(),
    };

    let mut files = Vec::new();
    let mut subdirs = Vec::new();

    for entry in entries.flatten() {
        let ft = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if ft.is_file() {
            let matched = if has_path_glob {
                let abs = entry.path();
                let rel = crate::path_helper::normalized_relative(root, &abs);
                glob::matches_any(&name_str, globs) || glob::matches_any(&rel, globs)
            } else {
                glob::matches_any(&name_str, globs)
            };
            if matched {
                files.push(entry.path().to_string_lossy().into_owned());
            }
        } else if ft.is_dir() && !filter.is_excluded(&name_str) {
            subdirs.push(entry.path());
        }
    }

    let sub_results: Vec<Vec<String>> = subdirs
        .par_iter()
        .map(|subdir| find_files_parallel(root, subdir, globs, has_path_glob, filter, cancelled))
        .collect();

    for sub in sub_results {
        files.extend(sub);
    }

    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_suffix_test() {
        assert!(is_test_file("foo_test.rs"));
        assert!(is_test_file("bar_test.go"));
        assert!(is_test_file("baz_test.py"));
    }

    #[test]
    fn test_file_suffix_spec() {
        assert!(is_test_file("foo_spec.ts"));
        assert!(is_test_file("bar_spec.rb"));
    }

    #[test]
    fn test_file_prefix() {
        assert!(is_test_file("test_utils.py"));
        assert!(is_test_file("test_main.rs"));
    }

    #[test]
    fn test_file_dot_test() {
        assert!(is_test_file("utils.test.ts"));
        assert!(is_test_file("app.test.tsx"));
    }

    #[test]
    fn test_file_dot_spec() {
        assert!(is_test_file("service.spec.ts"));
        assert!(is_test_file("handler.spec.js"));
    }

    #[test]
    fn test_file_directory_tests() {
        assert!(is_test_file("tests/unit/foo.rs"));
        assert!(is_test_file("__tests__/app.test.js"));
        assert!(is_test_file("spec/models/user_spec.rb"));
        assert!(is_test_file("test/main_test.go"));
    }

    #[test]
    fn test_file_case_insensitive() {
        assert!(is_test_file("FOO_TEST.RS"));
        assert!(is_test_file("Bar_Spec.ts"));
        assert!(is_test_file("TEST_utils.py"));
    }

    #[test]
    fn test_file_non_test() {
        assert!(!is_test_file("main.rs"));
        assert!(!is_test_file("utils.ts"));
        assert!(!is_test_file("app.py"));
        assert!(!is_test_file("src/lib.rs"));
        assert!(!is_test_file("attestation.rs"));
        assert!(!is_test_file("contest.py"));
        assert!(!is_test_file("latest_version.ts"));
    }

    #[test]
    fn test_file_windows_paths() {
        assert!(is_test_file("tests\\unit\\foo.rs"));
        assert!(is_test_file("__tests__\\app.test.js"));
    }

    #[test]
    fn test_file_specs_dir_excluded() {
        assert!(is_test_file("specs/requirement.md"));
    }
}
