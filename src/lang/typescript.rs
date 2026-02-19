use std::path::Path;
use super::LangImports;

pub struct TypeScriptImports;

impl LangImports for TypeScriptImports {
    fn extensions(&self) -> &[&str] {
        &["ts", "tsx", "js", "jsx", "mjs", "mts"]
    }

    fn extract_imports(&self, content: &str, file_path: &Path) -> Vec<String> {
        let mut imports = Vec::new();
        let file_dir = file_path.parent().unwrap_or_else(|| Path::new(""));

        for line in content.lines() {
            let trimmed = line.trim();

            if let Some(path) = extract_import_path(trimmed) {
                if path.starts_with("./") || path.starts_with("../") {
                    let resolved = resolve_relative(file_dir, path);
                    imports.extend(resolved);
                }
            }

            if let Some(path) = extract_require_path(trimmed) {
                if path.starts_with("./") || path.starts_with("../") {
                    let resolved = resolve_relative(file_dir, path);
                    imports.extend(resolved);
                }
            }
        }

        imports
    }
}

fn extract_import_path(line: &str) -> Option<&str> {
    if !line.starts_with("import ") && !line.starts_with("export ") {
        return None;
    }

    let from_idx = line.find(" from ")?;
    let after_from = &line[from_idx + 6..];
    extract_string_literal(after_from)
}

fn extract_require_path(line: &str) -> Option<&str> {
    let req_idx = line.find("require(")?;
    let after_req = &line[req_idx + 8..];
    extract_string_literal(after_req)
}

fn extract_string_literal(s: &str) -> Option<&str> {
    let s = s.trim();
    let (quote, rest) = if s.starts_with('\'') {
        ('\'', &s[1..])
    } else if s.starts_with('"') {
        ('"', &s[1..])
    } else {
        return None;
    };

    let end = rest.find(quote)?;
    let path = &rest[..end];
    if path.is_empty() { None } else { Some(path) }
}

fn resolve_relative(base: &Path, import_path: &str) -> Vec<String> {
    let resolved = base.join(import_path);
    let mut candidates = Vec::new();

    let base_str = normalize(&resolved);

    for ext in &[".ts", ".tsx", ".js", ".jsx", ".mjs", ".mts"] {
        candidates.push(format!("{}{}", base_str, ext));
    }

    candidates.push(format!("{}/index.ts", base_str));
    candidates.push(format!("{}/index.tsx", base_str));
    candidates.push(format!("{}/index.js", base_str));
    candidates.push(format!("{}/index.jsx", base_str));

    candidates
}

fn normalize(path: &Path) -> String {
    let s = path.to_string_lossy();
    if cfg!(windows) {
        s.replace('\\', "/")
    } else {
        s.into_owned()
    }
}
