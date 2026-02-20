use std::path::Path;
use super::{LangImports, LangSymbols, SymbolInfo};

pub struct RustImports;

impl LangImports for RustImports {
    fn extensions(&self) -> &[&str] {
        &["rs"]
    }

    fn extract_imports(&self, content: &str, file_path: &Path) -> Vec<String> {
        let mut imports = Vec::new();
        let file_dir = file_path.parent().unwrap_or_else(|| Path::new(""));

        for line in content.lines() {
            let trimmed = line.trim();

            if let Some(rest) = trimmed.strip_prefix("mod ") {
                if let Some(module_name) = parse_mod_decl(rest) {
                    if module_name != "tests" {
                        let sibling = file_dir.join(format!("{}.rs", module_name));
                        imports.push(normalize(sibling));
                        let subdir = file_dir.join(&module_name).join("mod.rs");
                        imports.push(normalize(subdir));
                    }
                }
            }

            if let Some(rest) = trimmed.strip_prefix("use crate::") {
                if let Some(path_part) = extract_use_path(rest) {
                    let segments: Vec<&str> = path_part.split("::").collect();
                    let resolved = resolve_crate_path(&segments);
                    imports.extend(resolved);
                }
            }

            if let Some(rest) = trimmed.strip_prefix("use super::") {
                if let Some(path_part) = extract_use_path(rest) {
                    let parent = file_dir.parent().unwrap_or_else(|| Path::new(""));
                    let segments: Vec<&str> = path_part.split("::").collect();
                    let resolved = resolve_relative_path(parent, &segments);
                    imports.extend(resolved);
                }
            }
        }

        imports
    }
}

impl LangSymbols for RustImports {
    fn extensions(&self) -> &[&str] {
        &["rs"]
    }

    fn extract_symbols(&self, content: &str) -> Vec<SymbolInfo> {
        let mut symbols = Vec::new();
        let mut current_parent: Option<String> = None;
        let mut impl_brace_depth: i32 = 0;
        let mut in_impl = false;

        for (line_idx, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            let line_num = line_idx + 1;

            if trimmed.is_empty() || trimmed.starts_with("//") {
                continue;
            }

            let (vis, rest) = extract_visibility(trimmed);

            if rest.starts_with("impl ") || rest.starts_with("impl<") {
                let type_name = extract_impl_type(rest);
                if let Some(name) = type_name {
                    current_parent = Some(name);
                    in_impl = true;
                    impl_brace_depth = 0;
                    for c in trimmed.chars() {
                        match c {
                            '{' => impl_brace_depth += 1,
                            '}' => impl_brace_depth -= 1,
                            _ => {}
                        }
                    }
                    continue;
                }
            }

            if in_impl {
                for c in trimmed.chars() {
                    match c {
                        '{' => impl_brace_depth += 1,
                        '}' => impl_brace_depth -= 1,
                        _ => {}
                    }
                }
                if impl_brace_depth <= 0 {
                    current_parent = None;
                    in_impl = false;
                }
            }

            if rest.starts_with("fn ") {
                if let Some(name) = extract_name_before_paren(rest, "fn ") {
                    let kind = if current_parent.is_some() { "method" } else { "fn" };
                    symbols.push(SymbolInfo {
                        kind,
                        name,
                        line: line_num,
                        visibility: vis,
                        parent: current_parent.clone(),
                        signature: make_signature(trimmed),
                    });
                }
                continue;
            }

            if let Some(name) = try_extract_keyword(rest, "struct ") {
                symbols.push(SymbolInfo {
                    kind: "struct",
                    name,
                    line: line_num,
                    visibility: vis,
                    parent: None,
                    signature: make_signature(trimmed),
                });
                continue;
            }

            if let Some(name) = try_extract_keyword(rest, "enum ") {
                symbols.push(SymbolInfo {
                    kind: "enum",
                    name,
                    line: line_num,
                    visibility: vis,
                    parent: None,
                    signature: make_signature(trimmed),
                });
                continue;
            }

            if let Some(name) = try_extract_keyword(rest, "trait ") {
                symbols.push(SymbolInfo {
                    kind: "trait",
                    name,
                    line: line_num,
                    visibility: vis,
                    parent: None,
                    signature: make_signature(trimmed),
                });
                continue;
            }

            if let Some(name) = try_extract_keyword(rest, "type ") {
                symbols.push(SymbolInfo {
                    kind: "type",
                    name,
                    line: line_num,
                    visibility: vis,
                    parent: None,
                    signature: make_signature(trimmed),
                });
                continue;
            }

            if let Some(name) = try_extract_keyword(rest, "const ") {
                symbols.push(SymbolInfo {
                    kind: "const",
                    name,
                    line: line_num,
                    visibility: vis,
                    parent: current_parent.clone(),
                    signature: make_signature(trimmed),
                });
                continue;
            }

            if rest.starts_with("mod ") {
                if let Some(module_name) = parse_mod_decl(&rest[4..]) {
                    symbols.push(SymbolInfo {
                        kind: "mod",
                        name: module_name.to_owned(),
                        line: line_num,
                        visibility: vis,
                        parent: None,
                        signature: make_signature(trimmed),
                    });
                }
            }
        }

        symbols
    }
}

fn extract_visibility(trimmed: &str) -> (Option<&'static str>, &str) {
    if let Some(rest) = trimmed.strip_prefix("pub(crate) ") {
        (Some("pub"), rest)
    } else if let Some(rest) = trimmed.strip_prefix("pub ") {
        (Some("pub"), rest)
    } else {
        (None, trimmed)
    }
}

fn extract_impl_type(rest: &str) -> Option<String> {
    let after_impl = if let Some(r) = rest.strip_prefix("impl<") {
        let close = r.find('>')?;
        r[close + 1..].trim_start()
    } else {
        rest.strip_prefix("impl ")?.trim_start()
    };

    let name_end = after_impl.find(|c: char| !c.is_alphanumeric() && c != '_')?;
    let name = &after_impl[..name_end];
    if name.is_empty() { None } else { Some(name.to_owned()) }
}

fn extract_name_before_paren(rest: &str, prefix: &str) -> Option<String> {
    let after = rest.strip_prefix(prefix)?;
    let paren = after.find('(')?;
    let name = after[..paren].trim();
    if name.is_empty() { None } else { Some(name.to_owned()) }
}

fn try_extract_keyword(rest: &str, keyword: &str) -> Option<String> {
    let after = rest.strip_prefix(keyword)?;
    let name_end = after.find(|c: char| !c.is_alphanumeric() && c != '_')?;
    let name = &after[..name_end];
    if name.is_empty() { None } else { Some(name.to_owned()) }
}

fn make_signature(trimmed: &str) -> String {
    if let Some(brace_pos) = trimmed.find('{') {
        trimmed[..=brace_pos].trim().to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn parse_mod_decl(rest: &str) -> Option<&str> {
    let rest = rest.trim();
    let name = rest.strip_suffix(';')?.trim();
    if name.is_empty() || name.contains(' ') || name.contains('{') {
        return None;
    }
    Some(name)
}

fn extract_use_path(rest: &str) -> Option<&str> {
    let end = rest.find(|c: char| c == ';' || c == '{' || c == ' ')?;
    let path = &rest[..end];
    if path.is_empty() { None } else { Some(path) }
}

fn resolve_crate_path(segments: &[&str]) -> Vec<String> {
    if segments.is_empty() {
        return Vec::new();
    }
    let mut results = Vec::new();
    let dir_path: String = segments.iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>()
        .join("/");

    results.push(format!("src/{}.rs", dir_path));
    if segments.len() > 1 {
        let parent: String = segments[..segments.len() - 1].join("/");
        results.push(format!("src/{}.rs", parent));
    }
    results
}

fn resolve_relative_path(base: &Path, segments: &[&str]) -> Vec<String> {
    if segments.is_empty() {
        return Vec::new();
    }
    let mut results = Vec::new();
    let dir_path: String = segments.iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>()
        .join("/");

    let full = base.join(format!("{}.rs", dir_path));
    results.push(normalize(full));

    if segments.len() > 1 {
        let parent: String = segments[..segments.len() - 1].join("/");
        let full = base.join(format!("{}.rs", parent));
        results.push(normalize(full));
    }
    results
}

fn normalize(path: std::path::PathBuf) -> String {
    let s = path.to_string_lossy();
    if cfg!(windows) {
        s.replace('\\', "/")
    } else {
        s.into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extract_imports(content: &str, file_path: &str) -> Vec<String> {
        RustImports.extract_imports(content, Path::new(file_path))
    }

    fn extract_syms(content: &str) -> Vec<SymbolInfo> {
        <RustImports as LangSymbols>::extract_symbols(&RustImports, content)
    }

    // ── Import Tests ──

    #[test]
    fn mod_declaration_generates_import() {
        let content = "mod cli;";
        let imports = extract_imports(content, "src/main.rs");
        assert!(imports.iter().any(|i| i.contains("cli.rs")));
    }

    #[test]
    fn mod_declaration_generates_subdir_import() {
        let content = "mod lang;";
        let imports = extract_imports(content, "src/main.rs");
        assert!(imports.iter().any(|i| i.contains("lang/mod.rs")));
    }

    #[test]
    fn mod_tests_is_skipped() {
        let content = "mod tests;";
        let imports = extract_imports(content, "src/main.rs");
        assert!(imports.is_empty());
    }

    #[test]
    fn use_crate_import() {
        let content = "use crate::models::FileEntry;";
        let imports = extract_imports(content, "src/searcher.rs");
        assert!(imports.iter().any(|i| i.contains("src/models")));
    }

    #[test]
    fn use_super_import() {
        let content = "use super::LangImports;";
        let imports = extract_imports(content, "src/lang/rust.rs");
        assert!(!imports.is_empty());
    }

    #[test]
    fn no_imports_for_plain_code() {
        let content = "fn main() { println!(\"hello\"); }";
        let imports = extract_imports(content, "src/main.rs");
        assert!(imports.is_empty());
    }

    #[test]
    fn multiple_mod_declarations() {
        let content = "mod cli;\nmod models;\nmod scanner;\n";
        let imports = extract_imports(content, "src/main.rs");
        assert!(imports.len() >= 6); // 2 candidates per mod (file + subdir)
    }

    // ── Symbol Tests ──

    #[test]
    fn extracts_pub_fn() {
        let content = "pub fn hello() {\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].kind, "fn");
        assert_eq!(syms[0].name, "hello");
        assert_eq!(syms[0].visibility, Some("pub"));
        assert_eq!(syms[0].line, 1);
    }

    #[test]
    fn extracts_private_fn() {
        let content = "fn private_fn() {\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].kind, "fn");
        assert_eq!(syms[0].visibility, None);
    }

    #[test]
    fn extracts_struct() {
        let content = "pub struct MyStruct {\n    field: i32,\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].kind, "struct");
        assert_eq!(syms[0].name, "MyStruct");
    }

    #[test]
    fn extracts_enum() {
        let content = "pub enum Color {\n    Red,\n    Blue,\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].kind, "enum");
        assert_eq!(syms[0].name, "Color");
    }

    #[test]
    fn extracts_trait() {
        let content = "pub trait Drawable {\n    fn draw(&self);\n}\n";
        let syms = extract_syms(content);
        let trait_sym = syms.iter().find(|s| s.kind == "trait").unwrap();
        assert_eq!(trait_sym.name, "Drawable");
    }

    #[test]
    fn extracts_type_alias() {
        let content = "pub type Result<T> = std::result::Result<T, Error>;\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].kind, "type");
        assert_eq!(syms[0].name, "Result");
    }

    #[test]
    fn extracts_const() {
        let content = "const MAX_SIZE: usize = 1024;\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].kind, "const");
        assert_eq!(syms[0].name, "MAX_SIZE");
    }

    #[test]
    fn extracts_mod() {
        let content = "mod utils;\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].kind, "mod");
        assert_eq!(syms[0].name, "utils");
    }

    #[test]
    fn method_inside_impl() {
        let content = r#"struct Foo;
impl Foo {
    pub fn bar(&self) -> i32 {
        42
    }
}
"#;
        let syms = extract_syms(content);
        let struct_sym = syms.iter().find(|s| s.kind == "struct").unwrap();
        assert_eq!(struct_sym.name, "Foo");
        let method_sym = syms.iter().find(|s| s.kind == "method").unwrap();
        assert_eq!(method_sym.name, "bar");
        assert_eq!(method_sym.parent, Some("Foo".to_owned()));
    }

    #[test]
    fn pub_crate_visibility() {
        let content = "pub(crate) fn internal() {}\n";
        let syms = extract_syms(content);
        assert_eq!(syms[0].visibility, Some("pub"));
    }

    #[test]
    fn skips_comments() {
        let content = "// pub fn commented_out() {}\nfn real() {}\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "real");
    }

    #[test]
    fn skips_empty_lines() {
        let content = "\n\n\nfn spaced() {}\n\n\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "spaced");
    }

    #[test]
    fn signature_truncated_at_brace() {
        let content = "pub fn hello(x: i32) {\n    x + 1\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms[0].signature, "pub fn hello(x: i32) {");
    }

    #[test]
    fn impl_with_generics() {
        let content = r#"impl<T> MyType {
    fn new() -> Self {
        Self
    }
}
"#;
        let syms = extract_syms(content);
        let method = syms.iter().find(|s| s.kind == "method").unwrap();
        assert_eq!(method.name, "new");
        assert_eq!(method.parent, Some("MyType".to_owned()));
    }

    #[test]
    fn multiple_items() {
        let content = r#"pub struct A;
pub struct B;
pub fn c() {}
pub enum D { X }
pub trait E {}
"#;
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 5);
    }
}
