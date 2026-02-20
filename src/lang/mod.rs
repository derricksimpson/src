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
