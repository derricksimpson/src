use std::path::Path;

use super::{LangImports, LangSymbols, SymbolInfo};

pub struct PythonImports;

struct ImportStatement {
    module: String,
    is_relative: bool,
    dot_count: usize,
}

impl LangImports for PythonImports {
    fn extensions(&self) -> &[&str] {
        &["py"]
    }

    fn extract_imports(&self, content: &str, file_path: &Path) -> Vec<String> {
        let statements = parse_python_imports(content);
        let file_dir = file_path.parent().unwrap_or_else(|| Path::new(""));

        let mut candidates = Vec::new();

        for stmt in &statements {
            if stmt.is_relative {
                let resolved = resolve_relative_import(file_dir, stmt.dot_count, &stmt.module);
                candidates.extend(resolved);
            } else {
                let resolved = resolve_absolute_import(&stmt.module);
                candidates.extend(resolved);
            }
        }

        candidates
    }
}

fn parse_python_imports(content: &str) -> Vec<ImportStatement> {
    let mut statements = Vec::new();
    let mut in_triple_double = false;
    let mut in_triple_single = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if in_triple_double {
            if trimmed.contains("\"\"\"") {
                in_triple_double = false;
            }
            continue;
        }
        if in_triple_single {
            if trimmed.contains("'''") {
                in_triple_single = false;
            }
            continue;
        }

        if trimmed.contains("\"\"\"") {
            let count = trimmed.matches("\"\"\"").count();
            if count == 1 {
                in_triple_double = true;
            }
            continue;
        }
        if trimmed.contains("'''") {
            let count = trimmed.matches("'''").count();
            if count == 1 {
                in_triple_single = true;
            }
            continue;
        }

        if trimmed.starts_with('#') {
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("from ") {
            let rest = rest.trim();
            if let Some(import_idx) = rest.find(" import ") {
                let module_part = rest[..import_idx].trim();
                let dot_count = module_part.chars().take_while(|&c| c == '.').count();

                if dot_count > 0 {
                    let sub_module = &module_part[dot_count..];
                    statements.push(ImportStatement {
                        module: sub_module.to_owned(),
                        is_relative: true,
                        dot_count,
                    });
                } else {
                    statements.push(ImportStatement {
                        module: module_part.to_owned(),
                        is_relative: false,
                        dot_count: 0,
                    });
                }
            }
        } else if let Some(rest) = trimmed.strip_prefix("import ") {
            let rest = rest.trim();
            let module = if let Some(as_idx) = rest.find(" as ") {
                &rest[..as_idx]
            } else {
                rest
            };
            let module = module.split(',').next().unwrap_or("").trim();
            if !module.is_empty() {
                statements.push(ImportStatement {
                    module: module.to_owned(),
                    is_relative: false,
                    dot_count: 0,
                });
            }
        }
    }

    statements
}

fn resolve_relative_import(file_dir: &Path, dot_count: usize, sub_module: &str) -> Vec<String> {
    let mut base = file_dir.to_path_buf();
    for _ in 0..(dot_count - 1) {
        base = base.parent().unwrap_or_else(|| Path::new("")).to_path_buf();
    }

    let module_path = if sub_module.is_empty() {
        base.clone()
    } else {
        base.join(sub_module.replace('.', "/"))
    };

    let path_str = normalize(&module_path);
    let mut candidates = Vec::new();
    candidates.push(format!("{}.py", path_str));
    candidates.push(format!("{}/__init__.py", path_str));
    candidates
}

fn resolve_absolute_import(module: &str) -> Vec<String> {
    let path = module.replace('.', "/");
    let mut candidates = Vec::new();
    candidates.push(format!("{}.py", path));
    candidates.push(format!("{}/__init__.py", path));

    let parts: Vec<&str> = module.split('.').collect();
    if parts.len() > 1 {
        for i in (1..parts.len()).rev() {
            let partial: String = parts[..i].join("/");
            candidates.push(format!("{}.py", partial));
            candidates.push(format!("{}/__init__.py", partial));
        }
    }

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

impl LangSymbols for PythonImports {
    fn extensions(&self) -> &[&str] {
        &["py"]
    }

    fn extract_symbols(&self, content: &str) -> Vec<SymbolInfo> {
        let mut symbols = Vec::new();
        let mut current_class: Option<(String, usize)> = None;

        for (line_idx, line) in content.lines().enumerate() {
            let line_num = line_idx + 1;
            let trimmed = line.trim();

            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('@') {
                continue;
            }

            let indent = line.len() - line.trim_start().len();

            if let Some((_, class_indent)) = &current_class {
                if indent <= *class_indent && !trimmed.is_empty() {
                    current_class = None;
                }
            }

            if trimmed.starts_with("class ") {
                if let Some(name) = extract_python_class_name(trimmed) {
                    current_class = Some((name.clone(), indent));
                    symbols.push(SymbolInfo {
                        kind: "class",
                        name,
                        line: line_num,
                        visibility: None,
                        parent: None,
                        signature: make_python_signature(trimmed),
                    });
                }
                continue;
            }

            let is_async = trimmed.starts_with("async ");
            let def_check = if is_async {
                trimmed.strip_prefix("async ").unwrap_or(trimmed)
            } else {
                trimmed
            };

            if def_check.starts_with("def ") {
                if let Some(name) = extract_python_func_name(def_check) {
                    let (kind, parent) = if let Some((ref class_name, class_indent)) = current_class {
                        if indent > class_indent {
                            ("method", Some(class_name.clone()))
                        } else {
                            ("fn", None)
                        }
                    } else {
                        ("fn", None)
                    };

                    symbols.push(SymbolInfo {
                        kind,
                        name,
                        line: line_num,
                        visibility: None,
                        parent,
                        signature: make_python_signature(trimmed),
                    });
                }
                continue;
            }

            if indent == 0 && current_class.is_none() {
                if let Some(name) = extract_python_const(trimmed) {
                    symbols.push(SymbolInfo {
                        kind: "const",
                        name,
                        line: line_num,
                        visibility: None,
                        parent: None,
                        signature: trimmed.to_owned(),
                    });
                }
            }
        }

        symbols
    }
}

fn extract_python_class_name(trimmed: &str) -> Option<String> {
    let after = trimmed.strip_prefix("class ")?;
    let name_end = after.find(|c: char| !c.is_alphanumeric() && c != '_')?;
    let name = &after[..name_end];
    if name.is_empty() { None } else { Some(name.to_owned()) }
}

fn extract_python_func_name(trimmed: &str) -> Option<String> {
    let after = trimmed.strip_prefix("def ")?;
    let paren = after.find('(')?;
    let name = after[..paren].trim();
    if name.is_empty() { None } else { Some(name.to_owned()) }
}

fn extract_python_const(trimmed: &str) -> Option<String> {
    let eq_pos = trimmed.find('=')?;
    if eq_pos == 0 {
        return None;
    }
    if trimmed.as_bytes().get(eq_pos.wrapping_sub(1)) == Some(&b'!') ||
       trimmed.as_bytes().get(eq_pos.wrapping_sub(1)) == Some(&b'<') ||
       trimmed.as_bytes().get(eq_pos.wrapping_sub(1)) == Some(&b'>') {
        return None;
    }
    if trimmed.as_bytes().get(eq_pos + 1) == Some(&b'=') {
        return None;
    }

    let name = trimmed[..eq_pos].trim();
    if name.is_empty() {
        return None;
    }
    if !name.chars().all(|c| c.is_uppercase() || c == '_' || c.is_ascii_digit()) {
        return None;
    }
    if name.starts_with('_') || name.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    Some(name.to_owned())
}

fn make_python_signature(trimmed: &str) -> String {
    if let Some(colon_pos) = trimmed.rfind(':') {
        trimmed[..=colon_pos].to_owned()
    } else {
        trimmed.to_owned()
    }
}
