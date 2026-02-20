use std::path::Path;
use super::{LangImports, LangSymbols, SymbolInfo};

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

impl LangSymbols for RustImports {
    fn extensions(&self) -> &[&str] {
        &["rs"]
    }

    fn extract_symbols(&self, content: &str) -> Vec<SymbolInfo> {
        let mut symbols = Vec::new();
        let mut current_parent: Option<String> = None;
        let mut impl_brace_depth: i32 = 0;
        let mut in_impl = false;

        for (line_idx, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            let line_num = line_idx + 1;

            if trimmed.is_empty() || trimmed.starts_with("//") {
                continue;
            }

            let (vis, rest) = extract_visibility(trimmed);

            if rest.starts_with("impl ") || rest.starts_with("impl<") {
                let type_name = extract_impl_type(rest);
                if let Some(name) = type_name {
                    current_parent = Some(name);
                    in_impl = true;
                    impl_brace_depth = 0;
                    for c in trimmed.chars() {
                        match c {
                            '{' => impl_brace_depth += 1,
                            '}' => impl_brace_depth -= 1,
                            _ => {}
                        }
                    }
                    continue;
                }
            }

            if in_impl {
                for c in trimmed.chars() {
                    match c {
                        '{' => impl_brace_depth += 1,
                        '}' => impl_brace_depth -= 1,
                        _ => {}
                    }
                }
                if impl_brace_depth <= 0 {
                    current_parent = None;
                    in_impl = false;
                }
            }

            if rest.starts_with("fn ") {
                if let Some(name) = extract_name_before_paren(rest, "fn ") {
                    let kind = if current_parent.is_some() { "method" } else { "fn" };
                    symbols.push(SymbolInfo {
                        kind,
                        name,
                        line: line_num,
                        visibility: vis,
                        parent: current_parent.clone(),
                        signature: make_signature(trimmed),
                    });
                }
                continue;
            }

            if let Some(name) = try_extract_keyword(rest, "struct ") {
                symbols.push(SymbolInfo {
                    kind: "struct",
                    name,
                    line: line_num,
                    visibility: vis,
                    parent: None,
                    signature: make_signature(trimmed),
                });
                continue;
            }

            if let Some(name) = try_extract_keyword(rest, "enum ") {
                symbols.push(SymbolInfo {
                    kind: "enum",
                    name,
                    line: line_num,
                    visibility: vis,
                    parent: None,
                    signature: make_signature(trimmed),
                });
                continue;
            }

            if let Some(name) = try_extract_keyword(rest, "trait ") {
                symbols.push(SymbolInfo {
                    kind: "trait",
                    name,
                    line: line_num,
                    visibility: vis,
                    parent: None,
                    signature: make_signature(trimmed),
                });
                continue;
            }

            if let Some(name) = try_extract_keyword(rest, "type ") {
                symbols.push(SymbolInfo {
                    kind: "type",
                    name,
                    line: line_num,
                    visibility: vis,
                    parent: None,
                    signature: make_signature(trimmed),
                });
                continue;
            }

            if let Some(name) = try_extract_keyword(rest, "const ") {
                symbols.push(SymbolInfo {
                    kind: "const",
                    name,
                    line: line_num,
                    visibility: vis,
                    parent: current_parent.clone(),
                    signature: make_signature(trimmed),
                });
                continue;
            }

            if rest.starts_with("mod ") {
                if let Some(module_name) = parse_mod_decl(&rest[4..]) {
                    symbols.push(SymbolInfo {
                        kind: "mod",
                        name: module_name.to_owned(),
                        line: line_num,
                        visibility: vis,
                        parent: None,
                        signature: make_signature(trimmed),
                    });
                }
            }
        }

        symbols
    }
}

fn extract_visibility(trimmed: &str) -> (Option<&'static str>, &str) {
    if let Some(rest) = trimmed.strip_prefix("pub(crate) ") {
        (Some("pub"), rest)
    } else if let Some(rest) = trimmed.strip_prefix("pub ") {
        (Some("pub"), rest)
    } else {
        (None, trimmed)
    }
}

fn extract_impl_type(rest: &str) -> Option<String> {
    let after_impl = if let Some(r) = rest.strip_prefix("impl<") {
        let close = r.find('>')?;
        r[close + 1..].trim_start()
    } else {
        rest.strip_prefix("impl ")?.trim_start()
    };

    let name_end = after_impl.find(|c: char| !c.is_alphanumeric() && c != '_')?;
    let name = &after_impl[..name_end];
    if name.is_empty() { None } else { Some(name.to_owned()) }
}

fn extract_name_before_paren(rest: &str, prefix: &str) -> Option<String> {
    let after = rest.strip_prefix(prefix)?;
    let paren = after.find('(')?;
    let name = after[..paren].trim();
    if name.is_empty() { None } else { Some(name.to_owned()) }
}

fn try_extract_keyword(rest: &str, keyword: &str) -> Option<String> {
    let after = rest.strip_prefix(keyword)?;
    let name_end = after.find(|c: char| !c.is_alphanumeric() && c != '_')?;
    let name = &after[..name_end];
    if name.is_empty() { None } else { Some(name.to_owned()) }
}

fn make_signature(trimmed: &str) -> String {
    if let Some(brace_pos) = trimmed.find('{') {
        trimmed[..=brace_pos].trim().to_owned()
    } else {
        trimmed.to_owned()
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
