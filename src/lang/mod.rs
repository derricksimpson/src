mod rust;
mod typescript;
mod csharp;
mod go;
mod python;

use std::path::Path;

pub trait LangImports: Sync {
    fn extensions(&self) -> &[&str];
    fn extract_imports(&self, content: &str, file_path: &Path) -> Vec<String>;
}

pub struct SymbolInfo {
    pub kind: &'static str,
    pub name: String,
    pub line: usize,
    pub visibility: Option<&'static str>,
    pub parent: Option<String>,
    pub signature: String,
}

pub trait LangSymbols: Sync {
    fn extensions(&self) -> &[&str];
    fn extract_symbols(&self, content: &str) -> Vec<SymbolInfo>;
}

static HANDLERS: &[&dyn LangImports] = &[
    &rust::RustImports,
    &typescript::TypeScriptImports,
    &csharp::CSharpImports,
    &go::GoImports,
    &python::PythonImports,
];

static SYMBOL_HANDLERS: &[&dyn LangSymbols] = &[
    &rust::RustImports,
    &typescript::TypeScriptImports,
    &csharp::CSharpImports,
    &go::GoImports,
    &python::PythonImports,
];

pub fn get_handler(extension: &str) -> Option<&'static dyn LangImports> {
    let ext_lower = extension.to_ascii_lowercase();
    for &handler in HANDLERS {
        for &supported in handler.extensions() {
            if supported == ext_lower {
                return Some(handler);
            }
        }
    }
    None
}

pub fn get_symbol_handler(extension: &str) -> Option<&'static dyn LangSymbols> {
    let ext_lower = extension.to_ascii_lowercase();
    for &handler in SYMBOL_HANDLERS {
        for &supported in handler.extensions() {
            if supported == ext_lower {
                return Some(handler);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_handler_rust() {
        assert!(get_handler("rs").is_some());
    }

    #[test]
    fn get_handler_typescript() {
        assert!(get_handler("ts").is_some());
        assert!(get_handler("tsx").is_some());
    }

    #[test]
    fn get_handler_javascript() {
        assert!(get_handler("js").is_some());
        assert!(get_handler("jsx").is_some());
        assert!(get_handler("mjs").is_some());
        assert!(get_handler("mts").is_some());
    }

    #[test]
    fn get_handler_csharp() {
        assert!(get_handler("cs").is_some());
    }

    #[test]
    fn get_handler_go() {
        assert!(get_handler("go").is_some());
    }

    #[test]
    fn get_handler_python() {
        assert!(get_handler("py").is_some());
    }

    #[test]
    fn get_handler_unknown_returns_none() {
        assert!(get_handler("xyz").is_none());
        assert!(get_handler("").is_none());
        assert!(get_handler("java").is_none());
    }

    #[test]
    fn get_handler_case_insensitive() {
        assert!(get_handler("RS").is_some());
        assert!(get_handler("Ts").is_some());
        assert!(get_handler("PY").is_some());
        assert!(get_handler("Go").is_some());
        assert!(get_handler("CS").is_some());
    }

    #[test]
    fn get_symbol_handler_rust() {
        assert!(get_symbol_handler("rs").is_some());
    }

    #[test]
    fn get_symbol_handler_typescript() {
        assert!(get_symbol_handler("ts").is_some());
        assert!(get_symbol_handler("tsx").is_some());
    }

    #[test]
    fn get_symbol_handler_csharp() {
        assert!(get_symbol_handler("cs").is_some());
    }

    #[test]
    fn get_symbol_handler_go() {
        assert!(get_symbol_handler("go").is_some());
    }

    #[test]
    fn get_symbol_handler_python() {
        assert!(get_symbol_handler("py").is_some());
    }

    #[test]
    fn get_symbol_handler_unknown() {
        assert!(get_symbol_handler("xyz").is_none());
    }
}
