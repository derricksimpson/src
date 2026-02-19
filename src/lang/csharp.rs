use std::path::Path;
use super::LangImports;

pub struct CSharpImports;

impl LangImports for CSharpImports {
    fn extensions(&self) -> &[&str] {
        &["cs"]
    }

    fn extract_imports(&self, content: &str, _file_path: &Path) -> Vec<String> {
        let mut imports = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();

            if let Some(ns) = extract_using_namespace(trimmed) {
                if is_external_namespace(ns) {
                    continue;
                }
                let path = namespace_to_path(ns);
                imports.push(path);
            }
        }

        imports
    }
}

fn extract_using_namespace(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("using ")?;
    if rest.starts_with("static ") || rest.starts_with("var ") || rest.contains('=') {
        return None;
    }
    let ns = rest.strip_suffix(';')?.trim();
    if ns.is_empty() { None } else { Some(ns) }
}

fn is_external_namespace(ns: &str) -> bool {
    let external_prefixes = [
        "System", "Microsoft", "Newtonsoft", "NuGet",
        "Xunit", "Moq", "AutoMapper", "FluentValidation",
        "Serilog", "MediatR", "Polly", "Dapper",
    ];
    for prefix in &external_prefixes {
        if ns == *prefix || ns.starts_with(&format!("{}.", prefix)) {
            return true;
        }
    }
    false
}

fn namespace_to_path(ns: &str) -> String {
    let segments: Vec<&str> = ns.split('.').collect();
    if segments.len() <= 1 {
        return format!("{}/", ns);
    }
    let path_segments = &segments[1..];
    path_segments.join("/") + "/"
}
