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

#[cfg(test)]
mod tests {
    use super::*;

    fn extract_syms(content: &str) -> Vec<SymbolInfo> {
        <GoImports as LangSymbols>::extract_symbols(&GoImports, content)
    }

    // ── Symbol Tests ──

    #[test]
    fn extracts_simple_func() {
        let content = "func main() {\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].kind, "fn");
        assert_eq!(syms[0].name, "main");
        assert_eq!(syms[0].visibility, None);
    }

    #[test]
    fn extracts_exported_func() {
        let content = "func HandleRequest(w http.ResponseWriter, r *http.Request) {\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms[0].kind, "fn");
        assert_eq!(syms[0].name, "HandleRequest");
        assert_eq!(syms[0].visibility, Some("pub"));
    }

    #[test]
    fn extracts_method_with_receiver() {
        let content = "func (s *Server) Start() error {\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].kind, "method");
        assert_eq!(syms[0].name, "Start");
        assert_eq!(syms[0].parent, Some("Server".to_owned()));
    }

    #[test]
    fn extracts_method_value_receiver() {
        let content = "func (p Point) Distance() float64 {\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms[0].kind, "method");
        assert_eq!(syms[0].parent, Some("Point".to_owned()));
    }

    #[test]
    fn extracts_struct_type() {
        let content = "type Config struct {\n  Port int\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms[0].kind, "struct");
        assert_eq!(syms[0].name, "Config");
        assert_eq!(syms[0].visibility, Some("pub"));
    }

    #[test]
    fn extracts_interface_type() {
        let content = "type Reader interface {\n  Read(p []byte) (n int, err error)\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms[0].kind, "interface");
        assert_eq!(syms[0].name, "Reader");
    }

    #[test]
    fn extracts_type_alias() {
        let content = "type UserID string\n";
        let syms = extract_syms(content);
        assert_eq!(syms[0].kind, "type");
        assert_eq!(syms[0].name, "UserID");
    }

    #[test]
    fn extracts_standalone_const() {
        let content = "const MaxRetries = 3\n";
        let syms = extract_syms(content);
        assert_eq!(syms[0].kind, "const");
        assert_eq!(syms[0].name, "MaxRetries");
        assert_eq!(syms[0].visibility, Some("pub"));
    }

    #[test]
    fn extracts_standalone_var() {
        let content = "var globalConfig Config\n";
        let syms = extract_syms(content);
        assert_eq!(syms[0].kind, "var");
        assert_eq!(syms[0].name, "globalConfig");
        assert_eq!(syms[0].visibility, None);
    }

    #[test]
    fn extracts_const_block() {
        let content = "const (\n  StatusOK = 200\n  StatusNotFound = 404\n)\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 2);
        assert_eq!(syms[0].kind, "const");
        assert_eq!(syms[0].name, "StatusOK");
        assert_eq!(syms[1].name, "StatusNotFound");
    }

    #[test]
    fn extracts_var_block() {
        let content = "var (\n  mu sync.Mutex\n  count int\n)\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 2);
        assert_eq!(syms[0].kind, "var");
    }

    #[test]
    fn go_visibility_exported() {
        assert_eq!(go_visibility("Handler"), Some("pub"));
    }

    #[test]
    fn go_visibility_unexported() {
        assert_eq!(go_visibility("handler"), None);
    }

    #[test]
    fn skips_comments() {
        let content = "// func ignored() {}\nfunc real() {\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "real");
    }

    #[test]
    fn extract_receiver_type_pointer() {
        assert_eq!(extract_receiver_type("s *Server"), Some("Server".to_owned()));
    }

    #[test]
    fn extract_receiver_type_value() {
        assert_eq!(extract_receiver_type("p Point"), Some("Point".to_owned()));
    }

    #[test]
    fn extract_first_ident_basic() {
        assert_eq!(extract_first_ident("MaxRetries = 3"), Some("MaxRetries"));
    }

    #[test]
    fn extract_first_ident_empty() {
        assert_eq!(extract_first_ident(""), None);
    }

    #[test]
    fn make_go_signature_with_brace() {
        assert_eq!(make_go_signature("func main() {"), "func main() {");
    }

    #[test]
    fn make_go_signature_without_brace() {
        assert_eq!(make_go_signature("type Foo string"), "type Foo string");
    }

    #[test]
    fn parse_go_imports_single() {
        let content = "import \"fmt\"\n";
        let imports = parse_go_imports(content);
        assert_eq!(imports, vec!["fmt"]);
    }

    #[test]
    fn parse_go_imports_block() {
        let content = "import (\n  \"fmt\"\n  \"os\"\n)\n";
        let imports = parse_go_imports(content);
        assert_eq!(imports, vec!["fmt", "os"]);
    }

    #[test]
    fn parse_go_imports_with_alias() {
        let content = "import (\n  f \"fmt\"\n)\n";
        let imports = parse_go_imports(content);
        assert_eq!(imports, vec!["fmt"]);
    }

    #[test]
    fn parse_go_imports_skips_comments() {
        let content = "import (\n  // standard lib\n  \"fmt\"\n)\n";
        let imports = parse_go_imports(content);
        assert_eq!(imports, vec!["fmt"]);
    }

    #[test]
    fn parse_go_imports_empty_block() {
        let content = "import (\n)\n";
        let imports = parse_go_imports(content);
        assert!(imports.is_empty());
    }

    #[test]
    fn extract_quoted_path_basic() {
        assert_eq!(extract_quoted_path("\"fmt\""), Some("fmt"));
    }

    #[test]
    fn extract_quoted_path_with_alias() {
        assert_eq!(extract_quoted_path("f \"fmt\""), Some("fmt"));
    }

    #[test]
    fn extensions_returns_go() {
        let exts = <GoImports as LangImports>::extensions(&GoImports);
        assert_eq!(exts, &["go"]);
    }

    #[test]
    fn multiple_symbols_comprehensive() {
        let content = r#"package main

type Server struct {
    Port int
}

type Handler interface {
    Handle()
}

func NewServer() *Server {
    return &Server{}
}

func (s *Server) Start() error {
    return nil
}

const MaxConns = 100

var debug bool
"#;
        let syms = extract_syms(content);
        assert!(syms.iter().any(|s| s.kind == "struct" && s.name == "Server"));
        assert!(syms.iter().any(|s| s.kind == "interface" && s.name == "Handler"));
        assert!(syms.iter().any(|s| s.kind == "fn" && s.name == "NewServer"));
        assert!(syms.iter().any(|s| s.kind == "method" && s.name == "Start"));
        assert!(syms.iter().any(|s| s.kind == "const" && s.name == "MaxConns"));
        assert!(syms.iter().any(|s| s.kind == "var" && s.name == "debug"));
    }
}
