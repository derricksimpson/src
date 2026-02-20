use std::path::Path;
use std::sync::OnceLock;

use super::{LangImports, LangSymbols, SymbolInfo};

pub struct GoImports;

static GO_MODULE_PATH: OnceLock<Option<String>> = OnceLock::new();

fn get_module_path(file_path: &Path) -> Option<&str> {
    GO_MODULE_PATH
        .get_or_init(|| find_and_parse_go_mod(file_path))
        .as_deref()
}

fn find_and_parse_go_mod(file_path: &Path) -> Option<String> {
    let mut dir = if file_path.is_file() {
        file_path.parent()?.to_path_buf()
    } else {
        file_path.to_path_buf()
    };

    loop {
        let go_mod = dir.join("go.mod");
        if go_mod.is_file() {
            return parse_module_line(&go_mod);
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn parse_module_line(go_mod_path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(go_mod_path).ok()?;
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("module ") {
            let module = rest.trim();
            if !module.is_empty() {
                return Some(module.to_owned());
            }
        }
    }
    None
}

impl LangImports for GoImports {
    fn extensions(&self) -> &[&str] {
        &["go"]
    }

    fn extract_imports(&self, content: &str, file_path: &Path) -> Vec<String> {
        let module_path = match get_module_path(file_path) {
            Some(m) => m,
            None => return Vec::new(),
        };

        let raw_imports = parse_go_imports(content);
        let mut results = Vec::new();

        for imp in raw_imports {
            if let Some(rest) = imp.strip_prefix(module_path) {
                let rel = rest.strip_prefix('/').unwrap_or(rest);
                if !rel.is_empty() {
                    results.push(format!("{}/", rel));
                }
            }
        }

        results
    }
}

fn parse_go_imports(content: &str) -> Vec<String> {
    let mut imports = Vec::new();

    enum State {
        Normal,
        InBlock,
    }

    let mut state = State::Normal;

    for line in content.lines() {
        let trimmed = line.trim();

        match state {
            State::Normal => {
                if trimmed.starts_with("import (") {
                    state = State::InBlock;
                } else if trimmed.starts_with("import ") {
                    if let Some(path) = extract_quoted_path(trimmed) {
                        imports.push(path.to_owned());
                    }
                }
            }
            State::InBlock => {
                if trimmed == ")" || trimmed.starts_with(')') {
                    state = State::Normal;
                    continue;
                }
                if trimmed.is_empty() || trimmed.starts_with("//") {
                    continue;
                }
                if let Some(path) = extract_quoted_path(trimmed) {
                    imports.push(path.to_owned());
                }
            }
        }
    }

    imports
}

fn extract_quoted_path(line: &str) -> Option<&str> {
    let start = line.find('"')? + 1;
    let rest = &line[start..];
    let end = rest.find('"')?;
    let path = &rest[..end];
    if path.is_empty() { None } else { Some(path) }
}

impl LangSymbols for GoImports {
    fn extensions(&self) -> &[&str] {
        &["go"]
    }

    fn extract_symbols(&self, content: &str) -> Vec<SymbolInfo> {
        let mut symbols = Vec::new();
        let mut in_const_block = false;
        let mut in_var_block = false;
        let mut paren_depth: i32 = 0;

        for (line_idx, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            let line_num = line_idx + 1;

            if trimmed.is_empty() || trimmed.starts_with("//") {
                continue;
            }

            if in_const_block || in_var_block {
                for c in trimmed.chars() {
                    match c {
                        '(' => paren_depth += 1,
                        ')' => paren_depth -= 1,
                        _ => {}
                    }
                }
                if paren_depth <= 0 {
                    in_const_block = false;
                    in_var_block = false;
                    continue;
                }

                if trimmed.starts_with(')') {
                    continue;
                }

                let kind = if in_const_block { "const" } else { "var" };
                if let Some(name) = extract_block_var_name(trimmed) {
                    let vis = go_visibility(&name);
                    symbols.push(SymbolInfo {
                        kind,
                        name: name.to_owned(),
                        line: line_num,
                        visibility: vis,
                        parent: None,
                        signature: trimmed.to_owned(),
                    });
                }
                continue;
            }

            if trimmed.starts_with("func ") {
                if let Some(sym) = parse_go_func(trimmed, line_num) {
                    symbols.push(sym);
                }
                continue;
            }

            if trimmed.starts_with("type ") {
                if let Some(sym) = parse_go_type(trimmed, line_num) {
                    symbols.push(sym);
                }
                continue;
            }

            if trimmed.starts_with("const (") {
                in_const_block = true;
                paren_depth = 1;
                continue;
            }

            if trimmed.starts_with("var (") {
                in_var_block = true;
                paren_depth = 1;
                continue;
            }

            if trimmed.starts_with("const ") {
                let after = &trimmed[6..];
                if let Some(name) = extract_first_ident(after) {
                    let vis = go_visibility(name);
                    symbols.push(SymbolInfo {
                        kind: "const",
                        name: name.to_owned(),
                        line: line_num,
                        visibility: vis,
                        parent: None,
                        signature: make_go_signature(trimmed),
                    });
                }
                continue;
            }

            if trimmed.starts_with("var ") {
                let after = &trimmed[4..];
                if let Some(name) = extract_first_ident(after) {
                    let vis = go_visibility(name);
                    symbols.push(SymbolInfo {
                        kind: "var",
                        name: name.to_owned(),
                        line: line_num,
                        visibility: vis,
                        parent: None,
                        signature: make_go_signature(trimmed),
                    });
                }
            }
        }

        symbols
    }
}

fn parse_go_func(trimmed: &str, line_num: usize) -> Option<SymbolInfo> {
    let after_func = trimmed.strip_prefix("func ")?;

    if after_func.starts_with('(') {
        let close_paren = after_func.find(')')?;
        let receiver = &after_func[1..close_paren];
        let receiver_type = extract_receiver_type(receiver);
        let rest = after_func[close_paren + 1..].trim();
        let paren = rest.find('(')?;
        let name = rest[..paren].trim();
        if name.is_empty() {
            return None;
        }
        let vis = go_visibility(name);
        Some(SymbolInfo {
            kind: "method",
            name: name.to_owned(),
            line: line_num,
            visibility: vis,
            parent: receiver_type,
            signature: make_go_signature(trimmed),
        })
    } else {
        let paren = after_func.find('(')?;
        let name = after_func[..paren].trim();
        if name.is_empty() {
            return None;
        }
        let vis = go_visibility(name);
        Some(SymbolInfo {
            kind: "fn",
            name: name.to_owned(),
            line: line_num,
            visibility: vis,
            parent: None,
            signature: make_go_signature(trimmed),
        })
    }
}

fn extract_receiver_type(receiver: &str) -> Option<String> {
    let tokens: Vec<&str> = receiver.split_whitespace().collect();
    if let Some(last) = tokens.last() {
        let name = last.trim_start_matches('*');
        if !name.is_empty() {
            return Some(name.to_owned());
        }
    }
    None
}

fn parse_go_type(trimmed: &str, line_num: usize) -> Option<SymbolInfo> {
    let after_type = trimmed.strip_prefix("type ")?.trim();
    let name_end = after_type.find(|c: char| !c.is_alphanumeric() && c != '_')?;
    let name = &after_type[..name_end];
    if name.is_empty() {
        return None;
    }

    let rest = after_type[name_end..].trim();
    let kind = if rest.starts_with("struct") {
        "struct"
    } else if rest.starts_with("interface") {
        "interface"
    } else {
        "type"
    };

    let vis = go_visibility(name);
    Some(SymbolInfo {
        kind,
        name: name.to_owned(),
        line: line_num,
        visibility: vis,
        parent: None,
        signature: make_go_signature(trimmed),
    })
}

fn extract_block_var_name(trimmed: &str) -> Option<&str> {
    let ident_end = trimmed.find(|c: char| !c.is_alphanumeric() && c != '_').unwrap_or(trimmed.len());
    let name = &trimmed[..ident_end];
    if name.is_empty() || name == "_" { None } else { Some(name) }
}

fn extract_first_ident(s: &str) -> Option<&str> {
    let s = s.trim();
    let end = s.find(|c: char| !c.is_alphanumeric() && c != '_').unwrap_or(s.len());
    let name = &s[..end];
    if name.is_empty() { None } else { Some(name) }
}

fn go_visibility(name: &str) -> Option<&'static str> {
    if name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
        Some("pub")
    } else {
        None
    }
}

fn make_go_signature(trimmed: &str) -> String {
    if let Some(brace_pos) = trimmed.find('{') {
        trimmed[..=brace_pos].trim().to_owned()
    } else {
        trimmed.to_owned()
    }
}
