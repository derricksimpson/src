use std::path::Path;
use super::LangImports;

pub struct RustImports;

impl LangImports for RustImports {
    fn extensions(&self) -> &[&str] {
        &["rs"]
    }

    fn extract_imports(&self, content: &str, file_path: &Path) -> Vec<String> {
        let mut imports = Vec::new();
        let file_dir = file_path.parent().unwrap_or_else(|| Path::new(""));

        for line in content.lines() {
            let trimmed = line.trim();

            if let Some(rest) = trimmed.strip_prefix("mod ") {
                if let Some(module_name) = parse_mod_decl(rest) {
                    if module_name != "tests" {
                        let sibling = file_dir.join(format!("{}.rs", module_name));
                        imports.push(normalize(sibling));
                        let subdir = file_dir.join(&module_name).join("mod.rs");
                        imports.push(normalize(subdir));
                    }
                }
            }

            if let Some(rest) = trimmed.strip_prefix("use crate::") {
                if let Some(path_part) = extract_use_path(rest) {
                    let segments: Vec<&str> = path_part.split("::").collect();
                    let resolved = resolve_crate_path(&segments);
                    imports.extend(resolved);
                }
            }

            if let Some(rest) = trimmed.strip_prefix("use super::") {
                if let Some(path_part) = extract_use_path(rest) {
                    let parent = file_dir.parent().unwrap_or_else(|| Path::new(""));
                    let segments: Vec<&str> = path_part.split("::").collect();
                    let resolved = resolve_relative_path(parent, &segments);
                    imports.extend(resolved);
                }
            }
        }

        imports
    }
}

fn parse_mod_decl(rest: &str) -> Option<&str> {
    let rest = rest.trim();
    let name = rest.strip_suffix(';')?.trim();
    if name.is_empty() || name.contains(' ') || name.contains('{') {
        return None;
    }
    Some(name)
}

fn extract_use_path(rest: &str) -> Option<&str> {
    let end = rest.find(|c: char| c == ';' || c == '{' || c == ' ')?;
    let path = &rest[..end];
    if path.is_empty() { None } else { Some(path) }
}

fn resolve_crate_path(segments: &[&str]) -> Vec<String> {
    if segments.is_empty() {
        return Vec::new();
    }
    let mut results = Vec::new();
    let dir_path: String = segments.iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>()
        .join("/");

    results.push(format!("src/{}.rs", dir_path));
    if segments.len() > 1 {
        let parent: String = segments[..segments.len() - 1].join("/");
        results.push(format!("src/{}.rs", parent));
    }
    results
}

fn resolve_relative_path(base: &Path, segments: &[&str]) -> Vec<String> {
    if segments.is_empty() {
        return Vec::new();
    }
    let mut results = Vec::new();
    let dir_path: String = segments.iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>()
        .join("/");

    let full = base.join(format!("{}.rs", dir_path));
    results.push(normalize(full));

    if segments.len() > 1 {
        let parent: String = segments[..segments.len() - 1].join("/");
        let full = base.join(format!("{}.rs", parent));
        results.push(normalize(full));
    }
    results
}

fn normalize(path: std::path::PathBuf) -> String {
    let s = path.to_string_lossy();
    if cfg!(windows) {
        s.replace('\\', "/")
    } else {
        s.into_owned()
    }
}
