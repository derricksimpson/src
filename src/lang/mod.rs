mod rust;
mod typescript;
mod csharp;

use std::path::Path;

pub trait LangImports: Sync {
    fn extensions(&self) -> &[&str];
    fn extract_imports(&self, content: &str, file_path: &Path) -> Vec<String>;
}

static HANDLERS: &[&dyn LangImports] = &[
    &rust::RustImports,
    &typescript::TypeScriptImports,
    &csharp::CSharpImports,
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
