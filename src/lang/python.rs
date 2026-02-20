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

#[cfg(test)]
mod tests {
    use super::*;

    fn extract_imports(content: &str, file_path: &str) -> Vec<String> {
        PythonImports.extract_imports(content, Path::new(file_path))
    }

    fn extract_syms(content: &str) -> Vec<SymbolInfo> {
        <PythonImports as LangSymbols>::extract_symbols(&PythonImports, content)
    }

    // ── Import Tests ──

    #[test]
    fn absolute_import() {
        let content = "import os\n";
        let imports = extract_imports(content, "main.py");
        assert!(imports.iter().any(|i| i.contains("os")));
    }

    #[test]
    fn from_import_absolute() {
        let content = "from mypackage.module import something\n";
        let imports = extract_imports(content, "main.py");
        assert!(imports.iter().any(|i| i.contains("mypackage")));
    }

    #[test]
    fn relative_import_single_dot() {
        let content = "from .sibling import func\n";
        let imports = extract_imports(content, "pkg/main.py");
        assert!(!imports.is_empty());
        assert!(imports.iter().any(|i| i.contains("sibling")));
    }

    #[test]
    fn relative_import_double_dot() {
        let content = "from ..parent import func\n";
        let imports = extract_imports(content, "pkg/sub/main.py");
        assert!(!imports.is_empty());
    }

    #[test]
    fn relative_import_dot_only() {
        let content = "from . import something\n";
        let imports = extract_imports(content, "pkg/main.py");
        assert!(!imports.is_empty());
    }

    #[test]
    fn import_with_alias() {
        let content = "import numpy as np\n";
        let imports = extract_imports(content, "main.py");
        assert!(imports.iter().any(|i| i.contains("numpy")));
    }

    #[test]
    fn skips_comments() {
        let content = "# import os\nimport sys\n";
        let imports = extract_imports(content, "main.py");
        assert!(imports.iter().any(|i| i.contains("sys")));
        assert!(!imports.iter().any(|i| i.contains("os")));
    }

    #[test]
    fn skips_triple_double_quote_strings() {
        let content = "\"\"\"import os\n\"\"\"\nimport sys\n";
        let imports = extract_imports(content, "main.py");
        assert!(imports.iter().any(|i| i.contains("sys")));
        assert!(!imports.iter().any(|i| i.contains("os")));
    }

    #[test]
    fn skips_triple_single_quote_strings() {
        let content = "'''import os\n'''\nimport sys\n";
        let imports = extract_imports(content, "main.py");
        assert!(imports.iter().any(|i| i.contains("sys")));
    }

    #[test]
    fn generates_py_and_init_candidates() {
        let content = "import mymodule\n";
        let imports = extract_imports(content, "main.py");
        let has_py = imports.iter().any(|i| i.ends_with(".py") && i.contains("mymodule"));
        let has_init = imports.iter().any(|i| i.contains("__init__.py"));
        assert!(has_py);
        assert!(has_init);
    }

    #[test]
    fn multi_level_absolute_generates_partials() {
        let content = "from a.b.c import d\n";
        let imports = extract_imports(content, "main.py");
        assert!(imports.iter().any(|i| i.contains("a/b/c")));
        assert!(imports.iter().any(|i| i.contains("a/b")));
        assert!(imports.iter().any(|i| i.contains("a.py") || i.contains("a/")));
    }

    // ── Symbol Tests ──

    #[test]
    fn extracts_function() {
        let content = "def hello():\n    pass\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].kind, "fn");
        assert_eq!(syms[0].name, "hello");
        assert_eq!(syms[0].line, 1);
    }

    #[test]
    fn extracts_async_function() {
        let content = "async def fetch_data():\n    pass\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].kind, "fn");
        assert_eq!(syms[0].name, "fetch_data");
    }

    #[test]
    fn extracts_class() {
        let content = "class MyClass:\n    pass\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].kind, "class");
        assert_eq!(syms[0].name, "MyClass");
    }

    #[test]
    fn extracts_class_with_base() {
        let content = "class Child(Parent):\n    pass\n";
        let syms = extract_syms(content);
        assert_eq!(syms[0].kind, "class");
        assert_eq!(syms[0].name, "Child");
    }

    #[test]
    fn extracts_method_inside_class() {
        let content = "class Foo:\n    def bar(self):\n        pass\n";
        let syms = extract_syms(content);
        let method = syms.iter().find(|s| s.kind == "method").unwrap();
        assert_eq!(method.name, "bar");
        assert_eq!(method.parent, Some("Foo".to_owned()));
    }

    #[test]
    fn extracts_async_method_inside_class() {
        let content = "class Foo:\n    async def bar(self):\n        pass\n";
        let syms = extract_syms(content);
        let method = syms.iter().find(|s| s.kind == "method").unwrap();
        assert_eq!(method.name, "bar");
    }

    #[test]
    fn extracts_constant_all_caps() {
        let content = "MAX_SIZE = 1024\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].kind, "const");
        assert_eq!(syms[0].name, "MAX_SIZE");
    }

    #[test]
    fn ignores_non_constant_assignment() {
        let content = "my_var = 42\n";
        let syms = extract_syms(content);
        assert!(syms.is_empty());
    }

    #[test]
    fn ignores_comparison_operators() {
        let content = "if x == 5:\n    pass\n";
        let syms = extract_syms(content);
        assert!(syms.is_empty());
    }

    #[test]
    fn ignores_decorated_lines() {
        let content = "@decorator\ndef my_func():\n    pass\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "my_func");
    }

    #[test]
    fn function_after_class_is_fn_not_method() {
        let content = "class Foo:\n    def bar(self):\n        pass\n\ndef standalone():\n    pass\n";
        let syms = extract_syms(content);
        let standalone = syms.iter().find(|s| s.name == "standalone").unwrap();
        assert_eq!(standalone.kind, "fn");
        assert_eq!(standalone.parent, None);
    }

    #[test]
    fn constant_starting_with_underscore_ignored() {
        let content = "_PRIVATE = 42\n";
        let syms = extract_syms(content);
        assert!(syms.is_empty());
    }

    #[test]
    fn constant_starting_with_digit_ignored() {
        let content = "3ABC = 42\n";
        let syms = extract_syms(content);
        assert!(syms.is_empty());
    }

    #[test]
    fn skips_comments_and_empty_lines() {
        let content = "# comment\n\ndef func():\n    pass\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
    }

    #[test]
    fn signature_truncated_at_colon() {
        let content = "def hello(x: int) -> str:\n    return str(x)\n";
        let syms = extract_syms(content);
        assert!(syms[0].signature.ends_with(':'));
    }

    #[test]
    fn class_signature_truncated() {
        let content = "class Foo(Bar):\n    pass\n";
        let syms = extract_syms(content);
        assert!(syms[0].signature.ends_with(':'));
    }

    #[test]
    fn extensions_returns_py() {
        let exts = <PythonImports as LangImports>::extensions(&PythonImports);
        assert_eq!(exts, &["py"]);
    }

    #[test]
    fn comprehensive_python_file() {
        let content = r#"MAX_RETRIES = 3
DB_HOST = "localhost"

class Database:
    def __init__(self):
        pass

    def connect(self):
        pass

    async def query(self, sql):
        pass

class Cache(Database):
    def get(self, key):
        pass

def create_app():
    pass

async def run_server():
    pass
"#;
        let syms = extract_syms(content);
        assert!(syms.iter().any(|s| s.kind == "const" && s.name == "MAX_RETRIES"));
        assert!(syms.iter().any(|s| s.kind == "const" && s.name == "DB_HOST"));
        assert!(syms.iter().any(|s| s.kind == "class" && s.name == "Database"));
        assert!(syms.iter().any(|s| s.kind == "class" && s.name == "Cache"));
        assert!(syms.iter().any(|s| s.kind == "method" && s.name == "__init__"));
        assert!(syms.iter().any(|s| s.kind == "method" && s.name == "connect"));
        assert!(syms.iter().any(|s| s.kind == "method" && s.name == "query"));
        assert!(syms.iter().any(|s| s.kind == "method" && s.name == "get" && s.parent == Some("Cache".to_owned())));
        assert!(syms.iter().any(|s| s.kind == "fn" && s.name == "create_app"));
        assert!(syms.iter().any(|s| s.kind == "fn" && s.name == "run_server"));
    }

    #[test]
    fn extract_python_class_name_simple() {
        assert_eq!(extract_python_class_name("class Foo:"), Some("Foo".to_owned()));
    }

    #[test]
    fn extract_python_class_name_with_base() {
        assert_eq!(extract_python_class_name("class Foo(Bar):"), Some("Foo".to_owned()));
    }

    #[test]
    fn extract_python_func_name_simple() {
        assert_eq!(extract_python_func_name("def hello():"), Some("hello".to_owned()));
    }

    #[test]
    fn extract_python_func_name_with_args() {
        assert_eq!(extract_python_func_name("def hello(x, y):"), Some("hello".to_owned()));
    }

    #[test]
    fn extract_python_const_valid() {
        assert_eq!(extract_python_const("MAX_SIZE = 100"), Some("MAX_SIZE".to_owned()));
    }

    #[test]
    fn extract_python_const_not_all_caps() {
        assert_eq!(extract_python_const("my_var = 100"), None);
    }

    #[test]
    fn extract_python_const_comparison() {
        assert_eq!(extract_python_const("x == 5"), None);
    }
}
