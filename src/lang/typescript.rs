use std::path::Path;
use super::{LangImports, LangSymbols, SymbolInfo};

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

impl LangSymbols for TypeScriptImports {
    fn extensions(&self) -> &[&str] {
        &["ts", "tsx", "js", "jsx", "mjs", "mts"]
    }

    fn extract_symbols(&self, content: &str) -> Vec<SymbolInfo> {
        let mut symbols = Vec::new();
        let mut current_class: Option<String> = None;
        let mut class_brace_depth: i32 = 0;
        let mut in_class = false;

        for (line_idx, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            let line_num = line_idx + 1;

            if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with("*") {
                if in_class {
                    update_brace_depth(trimmed, &mut class_brace_depth);
                    if class_brace_depth <= 0 {
                        current_class = None;
                        in_class = false;
                    }
                }
                continue;
            }

            if in_class {
                update_brace_depth(trimmed, &mut class_brace_depth);
                if class_brace_depth <= 0 {
                    current_class = None;
                    in_class = false;
                    continue;
                }
            }

            let (vis, rest) = extract_ts_visibility(trimmed);

            if in_class {
                if let Some(sym) = try_class_member(rest, line_num, &current_class) {
                    let member_vis = if vis.is_some() {
                        vis
                    } else {
                        extract_member_visibility(rest)
                    };
                    symbols.push(SymbolInfo {
                        visibility: member_vis,
                        ..sym
                    });
                    continue;
                }
            }

            if let Some(name) = try_extract_function(rest) {
                symbols.push(SymbolInfo {
                    kind: "fn",
                    name,
                    line: line_num,
                    visibility: vis,
                    parent: None,
                    signature: make_ts_signature(trimmed),
                });
                continue;
            }

            if let Some(name) = try_extract_ts_keyword(rest, "class ") {
                symbols.push(SymbolInfo {
                    kind: "class",
                    name: name.clone(),
                    line: line_num,
                    visibility: vis,
                    parent: None,
                    signature: make_ts_signature(trimmed),
                });
                current_class = Some(name);
                in_class = true;
                class_brace_depth = 0;
                update_brace_depth(trimmed, &mut class_brace_depth);
                continue;
            }

            if let Some(name) = try_extract_ts_keyword(rest, "interface ") {
                symbols.push(SymbolInfo {
                    kind: "interface",
                    name,
                    line: line_num,
                    visibility: vis,
                    parent: None,
                    signature: make_ts_signature(trimmed),
                });
                continue;
            }

            if let Some(name) = try_extract_ts_keyword(rest, "type ") {
                symbols.push(SymbolInfo {
                    kind: "type",
                    name,
                    line: line_num,
                    visibility: vis,
                    parent: None,
                    signature: make_ts_signature(trimmed),
                });
                continue;
            }

            if let Some(name) = try_extract_ts_keyword(rest, "enum ") {
                symbols.push(SymbolInfo {
                    kind: "enum",
                    name,
                    line: line_num,
                    visibility: vis,
                    parent: None,
                    signature: make_ts_signature(trimmed),
                });
                continue;
            }

            if rest.starts_with("const ") || rest.starts_with("let ") || rest.starts_with("var ") {
                let keyword_len = if rest.starts_with("const ") { 6 } else if rest.starts_with("let ") { 4 } else { 4 };
                let after = &rest[keyword_len..];
                if let Some(eq_pos) = after.find('=') {
                    let name = after[..eq_pos].trim().trim_end_matches(|c: char| c == ':' || c == ' ' || c == '<' || c == '>');
                    let name = name.split(':').next().unwrap_or("").trim();
                    if !name.is_empty() && !name.contains(' ') {
                        let after_eq = after[eq_pos + 1..].trim();
                        let kind = if is_arrow_function(after_eq) { "fn" } else { "const" };
                        symbols.push(SymbolInfo {
                            kind,
                            name: name.to_owned(),
                            line: line_num,
                            visibility: vis,
                            parent: None,
                            signature: make_ts_signature(trimmed),
                        });
                    }
                }
                continue;
            }

            if rest == "default" || rest.starts_with("default ") {
                if vis == Some("export") {
                    symbols.push(SymbolInfo {
                        kind: "export",
                        name: "default".to_owned(),
                        line: line_num,
                        visibility: vis,
                        parent: None,
                        signature: make_ts_signature(trimmed),
                    });
                }
                continue;
            }
        }

        symbols
    }
}

fn extract_ts_visibility(trimmed: &str) -> (Option<&'static str>, &str) {
    if let Some(rest) = trimmed.strip_prefix("export default ") {
        (Some("export"), rest)
    } else if let Some(rest) = trimmed.strip_prefix("export ") {
        (Some("export"), rest)
    } else {
        (None, trimmed)
    }
}

fn extract_member_visibility(rest: &str) -> Option<&'static str> {
    if rest.starts_with("public ") { Some("public") }
    else if rest.starts_with("private ") { Some("private") }
    else if rest.starts_with("protected ") { Some("protected") }
    else { None }
}

fn try_extract_function(rest: &str) -> Option<String> {
    let check = if rest.starts_with("async ") {
        rest.strip_prefix("async ")?.trim_start()
    } else {
        rest
    };

    let after = check.strip_prefix("function ")?;
    let after = after.strip_prefix("* ").unwrap_or(after);
    let paren = after.find('(')?;
    let name = after[..paren].trim();
    if name.is_empty() { None } else { Some(name.to_owned()) }
}

fn try_extract_ts_keyword(rest: &str, keyword: &str) -> Option<String> {
    let after = if rest.starts_with("abstract ") {
        let r = rest.strip_prefix("abstract ")?;
        r.strip_prefix(keyword)?
    } else {
        rest.strip_prefix(keyword)?
    };
    let name_end = after.find(|c: char| !c.is_alphanumeric() && c != '_' && c != '$')?;
    let name = &after[..name_end];
    if name.is_empty() { None } else { Some(name.to_owned()) }
}

fn try_class_member(rest: &str, line_num: usize, class_name: &Option<String>) -> Option<SymbolInfo> {
    let cleaned = strip_member_modifiers(rest);

    if cleaned.starts_with("async ") || cleaned.starts_with("static ") || cleaned.starts_with("get ") || cleaned.starts_with("set ") {
        let parts: Vec<&str> = cleaned.splitn(2, ' ').collect();
        if parts.len() == 2 {
            let inner = parts[1].trim();
            if let Some(paren) = inner.find('(') {
                let name = inner[..paren].trim();
                if !name.is_empty() && !name.contains(' ') {
                    return Some(SymbolInfo {
                        kind: "method",
                        name: name.to_owned(),
                        line: line_num,
                        visibility: None,
                        parent: class_name.clone(),
                        signature: rest.to_owned(),
                    });
                }
            }
        }
    }

    if let Some(paren) = cleaned.find('(') {
        let before = cleaned[..paren].trim();
        if !before.is_empty() && !before.contains(' ') && !before.starts_with("if") && !before.starts_with("for") && !before.starts_with("while") && !before.starts_with("return") {
            return Some(SymbolInfo {
                kind: "method",
                name: before.to_owned(),
                line: line_num,
                visibility: None,
                parent: class_name.clone(),
                signature: rest.to_owned(),
            });
        }
    }

    None
}

fn strip_member_modifiers(rest: &str) -> &str {
    let mut s = rest;
    for modifier in &["public ", "private ", "protected ", "static ", "abstract ", "readonly ", "override "] {
        while let Some(r) = s.strip_prefix(modifier) {
            s = r;
        }
    }
    s
}

fn is_arrow_function(after_eq: &str) -> bool {
    after_eq.starts_with('(')
        || after_eq.starts_with("async (")
        || after_eq.starts_with("async(")
}

fn update_brace_depth(trimmed: &str, depth: &mut i32) {
    for c in trimmed.chars() {
        match c {
            '{' => *depth += 1,
            '}' => *depth -= 1,
            _ => {}
        }
    }
}

fn make_ts_signature(trimmed: &str) -> String {
    if let Some(brace_pos) = trimmed.find('{') {
        trimmed[..=brace_pos].trim().to_owned()
    } else {
        trimmed.to_owned()
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
