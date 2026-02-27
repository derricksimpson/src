use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::{LangImports, LangSymbols, SymbolInfo};
use super::common::{self, CommentTracker};

pub struct GoImports;

use std::sync::OnceLock;
static GO_MODULE_CACHE: OnceLock<Mutex<HashMap<PathBuf, Option<String>>>> = OnceLock::new();

fn get_module_path_owned(file_path: &Path) -> Option<String> {
    let cache = GO_MODULE_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let go_mod_dir = find_go_mod_dir(file_path)?;

    let mut map = cache.lock().unwrap();
    if let Some(cached) = map.get(&go_mod_dir) {
        return cached.clone();
    }

    let result = parse_module_line(&go_mod_dir.join("go.mod"));
    map.insert(go_mod_dir, result.clone());
    result
}

fn find_go_mod_dir(file_path: &Path) -> Option<PathBuf> {
    let mut dir = if file_path.is_file() {
        file_path.parent()?.to_path_buf()
    } else {
        file_path.to_path_buf()
    };

    loop {
        if dir.join("go.mod").is_file() {
            return Some(dir);
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
        let module_path = match get_module_path_owned(file_path) {
            Some(m) => m,
            None => return Vec::new(),
        };
        let module_path = module_path.as_str();

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
        let all_lines: Vec<&str> = content.lines().collect();
        let mut symbols = Vec::new();
        let mut in_const_block = false;
        let mut in_var_block = false;
        let mut paren_depth: i32 = 0;
        let mut comment_tracker = CommentTracker::new();

        for (line_idx, line) in all_lines.iter().enumerate() {
            let trimmed = line.trim();
            let line_num = line_idx + 1;

            if trimmed.is_empty() || comment_tracker.is_comment(trimmed, "//") {
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
                        end_line: line_num,
                        visibility: vis,
                        parent: None,
                        signature: trimmed.to_owned(),
                    });
                }
                continue;
            }

            if trimmed.starts_with("func ") {
                if let Some(mut sym) = parse_go_func(trimmed, line_num) {
                    sym.end_line = find_go_brace_end(&all_lines, line_idx);
                    symbols.push(sym);
                }
                continue;
            }

            if trimmed.starts_with("type ") {
                if let Some(mut sym) = parse_go_type(trimmed, line_num) {
                    sym.end_line = find_go_brace_end(&all_lines, line_idx);
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
                        end_line: line_num,
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
                        end_line: line_num,
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

fn find_go_brace_end(lines: &[&str], start_idx: usize) -> usize {
    common::find_brace_end(lines, start_idx)
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
            end_line: 0,
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
            end_line: 0,
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
        end_line: 0,
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
    common::make_signature_brace(trimmed)
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

    // ── Deep: Realistic full-file simulation ──

    #[test]
    fn realistic_http_server() {
        let content = r#"package server

import (
	"context"
	"fmt"
	"log"
	"net/http"
	"sync"
	"time"
)

const (
	DefaultPort    = 8080
	DefaultTimeout = 30
	MaxHeaderBytes = 1 << 20
)

var (
	mu       sync.Mutex
	instance *HTTPServer
)

type Config struct {
	Host         string
	Port         int
	ReadTimeout  time.Duration
	WriteTimeout time.Duration
}

type HTTPServer struct {
	config Config
	server *http.Server
	router *http.ServeMux
}

type Middleware interface {
	Wrap(next http.Handler) http.Handler
}

type Route struct {
	Method  string
	Path    string
	Handler http.HandlerFunc
}

func NewHTTPServer(config Config) *HTTPServer {
	return &HTTPServer{
		config: config,
		router: http.NewServeMux(),
	}
}

func (s *HTTPServer) Start() error {
	s.server = &http.Server{
		Addr:         fmt.Sprintf("%s:%d", s.config.Host, s.config.Port),
		Handler:      s.router,
		ReadTimeout:  s.config.ReadTimeout,
		WriteTimeout: s.config.WriteTimeout,
	}
	return s.server.ListenAndServe()
}

func (s *HTTPServer) Stop(ctx context.Context) error {
	return s.server.Shutdown(ctx)
}

func (s *HTTPServer) AddRoute(method, path string, handler http.HandlerFunc) {
	s.router.HandleFunc(path, handler)
}

func (s *HTTPServer) Use(mw Middleware) {
}

func healthCheck(w http.ResponseWriter, r *http.Request) {
	w.WriteHeader(http.StatusOK)
	fmt.Fprint(w, "ok")
}

func createRouter() *http.ServeMux {
	mux := http.NewServeMux()
	return mux
}

type ResponseWriter interface {
	Write(data []byte) (int, error)
	SetHeader(key, value string)
	StatusCode() int
}

type logLevel int

const (
	logDebug logLevel = iota
	logInfo
	logWarn
	logError
)
"#;
        let syms = extract_syms(content);

        let consts: Vec<_> = syms.iter().filter(|s| s.kind == "const").collect();
        assert!(consts.iter().any(|s| s.name == "DefaultPort" && s.visibility == Some("pub")));
        assert!(consts.iter().any(|s| s.name == "DefaultTimeout" && s.visibility == Some("pub")));
        assert!(consts.iter().any(|s| s.name == "MaxHeaderBytes" && s.visibility == Some("pub")));
        assert!(consts.iter().any(|s| s.name == "logDebug" && s.visibility.is_none()));
        assert!(consts.iter().any(|s| s.name == "logInfo"));
        assert!(consts.iter().any(|s| s.name == "logWarn"));
        assert!(consts.iter().any(|s| s.name == "logError"));

        let vars: Vec<_> = syms.iter().filter(|s| s.kind == "var").collect();
        assert!(vars.iter().any(|s| s.name == "mu"));
        assert!(vars.iter().any(|s| s.name == "instance"));

        let structs: Vec<_> = syms.iter().filter(|s| s.kind == "struct").collect();
        assert!(structs.iter().any(|s| s.name == "Config" && s.visibility == Some("pub")));
        assert!(structs.iter().any(|s| s.name == "HTTPServer" && s.visibility == Some("pub")));
        assert!(structs.iter().any(|s| s.name == "Route" && s.visibility == Some("pub")));

        let ifaces: Vec<_> = syms.iter().filter(|s| s.kind == "interface").collect();
        assert!(ifaces.iter().any(|s| s.name == "Middleware" && s.visibility == Some("pub")));
        assert!(ifaces.iter().any(|s| s.name == "ResponseWriter" && s.visibility == Some("pub")));

        let fns: Vec<_> = syms.iter().filter(|s| s.kind == "fn").collect();
        assert!(fns.iter().any(|s| s.name == "NewHTTPServer" && s.visibility == Some("pub")));
        assert!(fns.iter().any(|s| s.name == "healthCheck" && s.visibility.is_none()));
        assert!(fns.iter().any(|s| s.name == "createRouter" && s.visibility.is_none()));

        let methods: Vec<_> = syms.iter().filter(|s| s.kind == "method").collect();
        assert!(methods.iter().any(|s| s.name == "Start" && s.parent == Some("HTTPServer".to_owned())));
        assert!(methods.iter().any(|s| s.name == "Stop" && s.parent == Some("HTTPServer".to_owned())));
        assert!(methods.iter().any(|s| s.name == "AddRoute" && s.parent == Some("HTTPServer".to_owned())));
        assert!(methods.iter().any(|s| s.name == "Use" && s.parent == Some("HTTPServer".to_owned())));

        let types: Vec<_> = syms.iter().filter(|s| s.kind == "type").collect();
        assert!(types.iter().any(|s| s.name == "logLevel"));
    }

    #[test]
    fn multi_return_function() {
        let content = "func divide(a, b float64) (float64, error) {\n  return a / b, nil\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms[0].kind, "fn");
        assert_eq!(syms[0].name, "divide");
    }

    #[test]
    fn named_return_values() {
        let content = "func parseConfig(raw string) (config Config, err error) {\n  return\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms[0].name, "parseConfig");
    }

    #[test]
    fn init_function() {
        let content = "func init() {\n  log.Println(\"initializing\")\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].kind, "fn");
        assert_eq!(syms[0].name, "init");
        assert_eq!(syms[0].visibility, None);
    }

    #[test]
    fn variadic_function() {
        let content = "func Sum(numbers ...int) int {\n  return 0\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms[0].name, "Sum");
        assert_eq!(syms[0].visibility, Some("pub"));
    }

    #[test]
    fn pointer_and_value_receivers() {
        let content = r#"type Counter struct {
    count int
}

func (c Counter) GetCount() int {
    return c.count
}

func (c *Counter) Increment() {
    c.count++
}

func (c *Counter) Reset() {
    c.count = 0
}
"#;
        let syms = extract_syms(content);
        let methods: Vec<_> = syms.iter().filter(|s| s.kind == "method").collect();
        assert_eq!(methods.len(), 3);
        for m in &methods {
            assert_eq!(m.parent, Some("Counter".to_owned()));
        }
    }

    #[test]
    fn empty_interface() {
        let content = "type Any interface{}\n";
        let syms = extract_syms(content);
        assert_eq!(syms[0].kind, "interface");
        assert_eq!(syms[0].name, "Any");
    }

    #[test]
    fn func_type_alias() {
        let content = "type HandlerFunc func(http.ResponseWriter, *http.Request)\n";
        let syms = extract_syms(content);
        assert_eq!(syms[0].kind, "type");
        assert_eq!(syms[0].name, "HandlerFunc");
    }

    #[test]
    fn type_with_embedded_struct() {
        let content = "type EnhancedServer struct {\n  Server\n  Logger\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms[0].kind, "struct");
        assert_eq!(syms[0].name, "EnhancedServer");
    }

    #[test]
    fn const_block_with_iota() {
        let content = "const (\n    Sunday Weekday = iota\n    Monday\n    Tuesday\n    Wednesday\n    Thursday\n    Friday\n    Saturday\n)\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 7);
        assert!(syms.iter().all(|s| s.kind == "const"));
        assert_eq!(syms[0].name, "Sunday");
        assert_eq!(syms[6].name, "Saturday");
    }

    #[test]
    fn go_line_numbers_accurate() {
        let content = "package main\n\n// comment\n\nfunc first() {\n}\n\ntype Second struct {\n}\n";
        let syms = extract_syms(content);
        let first = syms.iter().find(|s| s.name == "first").unwrap();
        assert_eq!(first.line, 5);
        let second = syms.iter().find(|s| s.name == "Second").unwrap();
        assert_eq!(second.line, 8);
    }

    #[test]
    fn blank_import() {
        let content = "import (\n  _ \"net/http/pprof\"\n  . \"fmt\"\n)\n";
        let imports = parse_go_imports(content);
        assert_eq!(imports.len(), 2);
        assert!(imports.contains(&"net/http/pprof".to_owned()));
        assert!(imports.contains(&"fmt".to_owned()));
    }

    #[test]
    fn method_signature_includes_receiver() {
        let content = "func (s *Server) Start(ctx context.Context) error {\n  return nil\n}\n";
        let syms = extract_syms(content);
        assert!(syms[0].signature.starts_with("func (s *Server) Start"));
        assert!(syms[0].signature.contains("context.Context"));
    }

    #[test]
    fn var_with_function_type() {
        let content = "var DefaultErrorHandler func(err error)\n";
        let syms = extract_syms(content);
        assert_eq!(syms[0].kind, "var");
        assert_eq!(syms[0].name, "DefaultErrorHandler");
        assert_eq!(syms[0].visibility, Some("pub"));
    }

    #[test]
    fn multiple_import_statements() {
        let content = "import \"fmt\"\nimport \"os\"\nimport (\n  \"log\"\n  \"net/http\"\n)\n";
        let imports = parse_go_imports(content);
        assert_eq!(imports.len(), 4);
    }
}
