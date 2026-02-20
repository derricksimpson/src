use std::path::Path;
use super::{LangImports, LangSymbols, SymbolInfo};

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

impl LangSymbols for CSharpImports {
    fn extensions(&self) -> &[&str] {
        &["cs"]
    }

    fn extract_symbols(&self, content: &str) -> Vec<SymbolInfo> {
        let mut symbols = Vec::new();
        let mut current_class: Option<String> = None;
        let mut class_brace_depth: i32 = 0;
        let mut in_class = false;

        for (line_idx, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            let line_num = line_idx + 1;

            if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with("*") || trimmed == "{" || trimmed == "}" {
                if in_class {
                    update_brace_depth(trimmed, &mut class_brace_depth);
                    if class_brace_depth <= 0 {
                        current_class = None;
                        in_class = false;
                    }
                }
                continue;
            }

            if in_class {
                update_brace_depth(trimmed, &mut class_brace_depth);
                if class_brace_depth <= 0 {
                    current_class = None;
                    in_class = false;
                    continue;
                }
            }

            let (vis, rest) = extract_cs_visibility(trimmed);

            let rest_clean = strip_cs_modifiers(rest);

            if let Some(name) = try_cs_keyword(rest_clean, "namespace ") {
                symbols.push(SymbolInfo {
                    kind: "namespace",
                    name,
                    line: line_num,
                    visibility: vis,
                    parent: None,
                    signature: make_cs_signature(trimmed),
                });
                continue;
            }

            if let Some(name) = try_cs_keyword(rest_clean, "class ") {
                symbols.push(SymbolInfo {
                    kind: "class",
                    name: name.clone(),
                    line: line_num,
                    visibility: vis,
                    parent: None,
                    signature: make_cs_signature(trimmed),
                });
                current_class = Some(name);
                in_class = true;
                class_brace_depth = 0;
                update_brace_depth(trimmed, &mut class_brace_depth);
                continue;
            }

            if let Some(name) = try_cs_keyword(rest_clean, "interface ") {
                symbols.push(SymbolInfo {
                    kind: "interface",
                    name,
                    line: line_num,
                    visibility: vis,
                    parent: None,
                    signature: make_cs_signature(trimmed),
                });
                continue;
            }

            if let Some(name) = try_cs_keyword(rest_clean, "struct ") {
                symbols.push(SymbolInfo {
                    kind: "struct",
                    name,
                    line: line_num,
                    visibility: vis,
                    parent: None,
                    signature: make_cs_signature(trimmed),
                });
                continue;
            }

            if let Some(name) = try_cs_keyword(rest_clean, "enum ") {
                symbols.push(SymbolInfo {
                    kind: "enum",
                    name,
                    line: line_num,
                    visibility: vis,
                    parent: None,
                    signature: make_cs_signature(trimmed),
                });
                continue;
            }

            if rest_clean.starts_with("const ") {
                if let Some(name) = extract_cs_const(rest_clean) {
                    symbols.push(SymbolInfo {
                        kind: "const",
                        name,
                        line: line_num,
                        visibility: vis,
                        parent: current_class.clone(),
                        signature: make_cs_signature(trimmed),
                    });
                    continue;
                }
            }

            if in_class {
                if let Some(name) = try_cs_method(rest_clean) {
                    symbols.push(SymbolInfo {
                        kind: "method",
                        name,
                        line: line_num,
                        visibility: vis,
                        parent: current_class.clone(),
                        signature: make_cs_signature(trimmed),
                    });
                }
            }
        }

        symbols
    }
}

fn extract_cs_visibility(trimmed: &str) -> (Option<&'static str>, &str) {
    if let Some(rest) = trimmed.strip_prefix("public ") {
        (Some("public"), rest)
    } else if let Some(rest) = trimmed.strip_prefix("private ") {
        (Some("private"), rest)
    } else if let Some(rest) = trimmed.strip_prefix("protected internal ") {
        (Some("protected internal"), rest)
    } else if let Some(rest) = trimmed.strip_prefix("protected ") {
        (Some("protected"), rest)
    } else if let Some(rest) = trimmed.strip_prefix("internal ") {
        (Some("internal"), rest)
    } else {
        (None, trimmed)
    }
}

fn strip_cs_modifiers(rest: &str) -> &str {
    let mut s = rest;
    for modifier in &["static ", "abstract ", "virtual ", "override ", "sealed ", "async ", "partial ", "readonly ", "new ", "extern "] {
        while let Some(r) = s.strip_prefix(modifier) {
            s = r;
        }
    }
    s
}

fn try_cs_keyword(rest: &str, keyword: &str) -> Option<String> {
    let after = rest.strip_prefix(keyword)?;
    let name_end = after.find(|c: char| !c.is_alphanumeric() && c != '_')?;
    let name = &after[..name_end];
    if name.is_empty() { None } else { Some(name.to_owned()) }
}

fn extract_cs_const(rest: &str) -> Option<String> {
    let after = rest.strip_prefix("const ")?;
    let tokens: Vec<&str> = after.split_whitespace().collect();
    if tokens.len() >= 2 {
        let name = tokens[1].trim_end_matches(|c: char| c == '=' || c == ';');
        if !name.is_empty() { Some(name.to_owned()) } else { None }
    } else {
        None
    }
}

fn try_cs_method(rest: &str) -> Option<String> {
    if rest.starts_with("class ") || rest.starts_with("interface ") || rest.starts_with("struct ") || rest.starts_with("enum ") || rest.starts_with("namespace ") {
        return None;
    }
    if rest.starts_with("using ") || rest.starts_with("return ") || rest.starts_with("if ") || rest.starts_with("for ") || rest.starts_with("foreach ") || rest.starts_with("while ") || rest.starts_with("switch ") {
        return None;
    }
    if rest.starts_with("const ") || rest.starts_with("var ") {
        return None;
    }

    let paren = rest.find('(')?;
    let before = rest[..paren].trim();

    let tokens: Vec<&str> = before.split_whitespace().collect();
    if tokens.len() >= 2 {
        let method_name = tokens[tokens.len() - 1];
        if method_name.chars().next()?.is_alphabetic() && !method_name.contains('.') {
            return Some(method_name.to_owned());
        }
    }
    None
}

fn update_brace_depth(trimmed: &str, depth: &mut i32) {
    for c in trimmed.chars() {
        match c {
            '{' => *depth += 1,
            '}' => *depth -= 1,
            _ => {}
        }
    }
}

fn make_cs_signature(trimmed: &str) -> String {
    if let Some(brace_pos) = trimmed.find('{') {
        trimmed[..=brace_pos].trim().to_owned()
    } else {
        trimmed.to_owned()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn extract_imports(content: &str) -> Vec<String> {
        CSharpImports.extract_imports(content, Path::new("Foo.cs"))
    }

    fn extract_syms(content: &str) -> Vec<SymbolInfo> {
        <CSharpImports as LangSymbols>::extract_symbols(&CSharpImports, content)
    }

    // ── Import Tests ──

    #[test]
    fn using_project_namespace() {
        let content = "using MyApp.Services;\n";
        let imports = extract_imports(content);
        assert!(!imports.is_empty());
        assert!(imports.iter().any(|i| i.contains("Services/")));
    }

    #[test]
    fn using_system_ignored() {
        let content = "using System;\nusing System.Collections.Generic;\n";
        let imports = extract_imports(content);
        assert!(imports.is_empty());
    }

    #[test]
    fn using_microsoft_ignored() {
        let content = "using Microsoft.Extensions.DependencyInjection;\n";
        let imports = extract_imports(content);
        assert!(imports.is_empty());
    }

    #[test]
    fn using_newtonsoft_ignored() {
        let content = "using Newtonsoft.Json;\n";
        let imports = extract_imports(content);
        assert!(imports.is_empty());
    }

    #[test]
    fn using_static_ignored() {
        let content = "using static System.Math;\n";
        let imports = extract_imports(content);
        assert!(imports.is_empty());
    }

    #[test]
    fn using_alias_ignored() {
        let content = "using Alias = MyApp.Services.Foo;\n";
        let imports = extract_imports(content);
        assert!(imports.is_empty());
    }

    #[test]
    fn single_segment_namespace() {
        let content = "using LocalNamespace;\n";
        let imports = extract_imports(content);
        assert!(imports.iter().any(|i| i == "LocalNamespace/"));
    }

    // ── Symbol Tests ──

    #[test]
    fn extracts_namespace() {
        let content = "namespace MyApp.Services\n{\n}\n";
        let syms = extract_syms(content);
        let ns = syms.iter().find(|s| s.kind == "namespace").unwrap();
        assert_eq!(ns.name, "MyApp");
    }

    #[test]
    fn extracts_class() {
        let content = "public class UserService {\n}\n";
        let syms = extract_syms(content);
        let cls = syms.iter().find(|s| s.kind == "class").unwrap();
        assert_eq!(cls.name, "UserService");
        assert_eq!(cls.visibility, Some("public"));
    }

    #[test]
    fn extracts_interface() {
        let content = "public interface IRepository {\n}\n";
        let syms = extract_syms(content);
        let iface = syms.iter().find(|s| s.kind == "interface").unwrap();
        assert_eq!(iface.name, "IRepository");
    }

    #[test]
    fn extracts_struct() {
        let content = "public struct Point {\n}\n";
        let syms = extract_syms(content);
        let st = syms.iter().find(|s| s.kind == "struct").unwrap();
        assert_eq!(st.name, "Point");
    }

    #[test]
    fn extracts_enum() {
        let content = "public enum Color {\n  Red,\n  Blue,\n}\n";
        let syms = extract_syms(content);
        let en = syms.iter().find(|s| s.kind == "enum").unwrap();
        assert_eq!(en.name, "Color");
    }

    #[test]
    fn extracts_method_inside_class() {
        let content = "public class Foo {\n  public void Bar() {\n  }\n}\n";
        let syms = extract_syms(content);
        let method = syms.iter().find(|s| s.kind == "method").unwrap();
        assert_eq!(method.name, "Bar");
        assert_eq!(method.parent, Some("Foo".to_owned()));
    }

    #[test]
    fn extracts_const() {
        let content = "public class Foo {\n  public const int MaxSize = 100;\n}\n";
        let syms = extract_syms(content);
        let c = syms.iter().find(|s| s.kind == "const").unwrap();
        assert_eq!(c.name, "MaxSize");
    }

    #[test]
    fn private_visibility() {
        let content = "private class Internal {\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms[0].visibility, Some("private"));
    }

    #[test]
    fn protected_visibility() {
        let content = "protected class Base {\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms[0].visibility, Some("protected"));
    }

    #[test]
    fn internal_visibility() {
        let content = "internal class Helper {\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms[0].visibility, Some("internal"));
    }

    #[test]
    fn static_class() {
        let content = "public static class Utils {\n}\n";
        let syms = extract_syms(content);
        let cls = syms.iter().find(|s| s.kind == "class").unwrap();
        assert_eq!(cls.name, "Utils");
    }

    #[test]
    fn abstract_class() {
        let content = "public abstract class BaseService {\n}\n";
        let syms = extract_syms(content);
        let cls = syms.iter().find(|s| s.kind == "class").unwrap();
        assert_eq!(cls.name, "BaseService");
    }

    #[test]
    fn async_method() {
        let content = "public class Foo {\n  public async Task DoWork() {\n  }\n}\n";
        let syms = extract_syms(content);
        let method = syms.iter().find(|s| s.kind == "method").unwrap();
        assert_eq!(method.name, "DoWork");
    }

    #[test]
    fn skips_comments() {
        let content = "// public class Ignored {}\npublic class Real {\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "Real");
    }

    #[test]
    fn skips_using_inside_class() {
        let content = "public class Foo {\n  using var stream = new MemoryStream();\n}\n";
        let syms = extract_syms(content);
        let methods: Vec<_> = syms.iter().filter(|s| s.kind == "method").collect();
        assert!(methods.is_empty());
    }

    #[test]
    fn extensions_returns_cs() {
        let exts = <CSharpImports as LangImports>::extensions(&CSharpImports);
        assert_eq!(exts, &["cs"]);
    }

    #[test]
    fn namespace_to_path_single() {
        assert_eq!(namespace_to_path("MyApp"), "MyApp/");
    }

    #[test]
    fn namespace_to_path_multi() {
        assert_eq!(namespace_to_path("MyApp.Services.Auth"), "Services/Auth/");
    }

    #[test]
    fn is_external_system() {
        assert!(is_external_namespace("System"));
        assert!(is_external_namespace("System.Collections.Generic"));
    }

    #[test]
    fn is_external_false() {
        assert!(!is_external_namespace("MyApp"));
        assert!(!is_external_namespace("MyApp.Services"));
    }

    #[test]
    fn extract_using_namespace_valid() {
        assert_eq!(extract_using_namespace("using MyApp.Services;"), Some("MyApp.Services"));
    }

    #[test]
    fn extract_using_namespace_static_returns_none() {
        assert_eq!(extract_using_namespace("using static System.Math;"), None);
    }

    #[test]
    fn extract_using_namespace_alias_returns_none() {
        assert_eq!(extract_using_namespace("using Alias = Some.Namespace;"), None);
    }
}
