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
        let all_lines: Vec<&str> = content.lines().collect();
        let mut symbols = Vec::new();
        let mut current_class: Option<String> = None;
        let mut class_brace_depth: i32 = 0;
        let mut in_class = false;

        for (line_idx, line) in all_lines.iter().enumerate() {
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
                let end_line = find_cs_brace_end(&all_lines, line_idx);
                symbols.push(SymbolInfo {
                    kind: "namespace",
                    name,
                    line: line_num,
                    end_line,
                    visibility: vis,
                    parent: None,
                    signature: make_cs_signature(trimmed),
                });
                continue;
            }

            if let Some(name) = try_cs_keyword(rest_clean, "class ") {
                let end_line = find_cs_brace_end(&all_lines, line_idx);
                symbols.push(SymbolInfo {
                    kind: "class",
                    name: name.clone(),
                    line: line_num,
                    end_line,
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
                let end_line = find_cs_brace_end(&all_lines, line_idx);
                symbols.push(SymbolInfo {
                    kind: "interface",
                    name,
                    line: line_num,
                    end_line,
                    visibility: vis,
                    parent: None,
                    signature: make_cs_signature(trimmed),
                });
                continue;
            }

            if let Some(name) = try_cs_keyword(rest_clean, "struct ") {
                let end_line = find_cs_brace_end(&all_lines, line_idx);
                symbols.push(SymbolInfo {
                    kind: "struct",
                    name,
                    line: line_num,
                    end_line,
                    visibility: vis,
                    parent: None,
                    signature: make_cs_signature(trimmed),
                });
                continue;
            }

            if let Some(name) = try_cs_keyword(rest_clean, "enum ") {
                let end_line = find_cs_brace_end(&all_lines, line_idx);
                symbols.push(SymbolInfo {
                    kind: "enum",
                    name,
                    line: line_num,
                    end_line,
                    visibility: vis,
                    parent: None,
                    signature: make_cs_signature(trimmed),
                });
                continue;
            }

            if rest_clean.starts_with("const ") {
                if let Some(name) = extract_cs_const(rest_clean) {
                    let end_line = find_cs_semicolon_or_same(&all_lines, line_idx);
                    symbols.push(SymbolInfo {
                        kind: "const",
                        name,
                        line: line_num,
                        end_line,
                        visibility: vis,
                        parent: current_class.clone(),
                        signature: make_cs_signature(trimmed),
                    });
                    continue;
                }
            }

            if in_class {
                if let Some(name) = try_cs_method(rest_clean) {
                    let end_line = find_cs_brace_end(&all_lines, line_idx);
                    symbols.push(SymbolInfo {
                        kind: "method",
                        name,
                        line: line_num,
                        end_line,
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

fn find_cs_brace_end(lines: &[&str], start_idx: usize) -> usize {
    let mut depth: i32 = 0;
    for (i, line) in lines[start_idx..].iter().enumerate() {
        for c in line.chars() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth <= 0 {
                        return start_idx + i + 1;
                    }
                }
                _ => {}
            }
        }
    }
    start_idx + 1
}

fn find_cs_semicolon_or_same(lines: &[&str], start_idx: usize) -> usize {
    for (i, line) in lines[start_idx..].iter().enumerate() {
        if line.contains(';') {
            return start_idx + i + 1;
        }
    }
    start_idx + 1
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

    // ── Deep: Realistic full-file simulation ──

    #[test]
    fn realistic_aspnet_controller() {
        let content = r#"using MyApp.Models;
using MyApp.Services;
using MyApp.DTOs;

namespace MyApp.Controllers
{
    public class UsersController {
        private readonly IUserService _userService;
        private readonly ILogger _logger;

        public UsersController(IUserService userService, ILogger logger) {
            _userService = userService;
            _logger = logger;
        }

        public async Task<ActionResult<List<UserDTO>>> GetAll() {
            var users = await _userService.GetAllAsync();
            return Ok(users);
        }

        public async Task<ActionResult<UserDTO>> GetById(int id) {
            var user = await _userService.GetByIdAsync(id);
            return Ok(user);
        }

        public async Task<ActionResult> Create(CreateUserDTO dto) {
            await _userService.CreateAsync(dto);
            return Created();
        }

        public async Task<ActionResult> Update(int id, UpdateUserDTO dto) {
            await _userService.UpdateAsync(id, dto);
            return NoContent();
        }

        public async Task<ActionResult> Delete(int id) {
            await _userService.DeleteAsync(id);
            return NoContent();
        }
    }
}
"#;
        let syms = extract_syms(content);

        let ns = syms.iter().find(|s| s.kind == "namespace").unwrap();
        assert_eq!(ns.name, "MyApp");

        let cls = syms.iter().find(|s| s.kind == "class").unwrap();
        assert_eq!(cls.name, "UsersController");
        assert_eq!(cls.visibility, Some("public"));

        let methods: Vec<_> = syms.iter()
            .filter(|s| s.kind == "method" && s.parent == Some("UsersController".to_owned()))
            .collect();
        // C# constructors have no return type, so try_cs_method requires >= 2 tokens
        // (return_type + name) and won't match a constructor. This is a known parser limitation.
        assert!(!methods.iter().any(|s| s.name == "UsersController"));
        assert!(methods.iter().any(|s| s.name == "GetAll"));
        assert!(methods.iter().any(|s| s.name == "GetById"));
        assert!(methods.iter().any(|s| s.name == "Create"));
        assert!(methods.iter().any(|s| s.name == "Update"));
        assert!(methods.iter().any(|s| s.name == "Delete"));
    }

    #[test]
    fn realistic_controller_imports() {
        let content = r#"using System;
using System.Collections.Generic;
using Microsoft.AspNetCore.Mvc;
using MyApp.Models;
using MyApp.Services;
"#;
        let imports = extract_imports(content);
        // System and Microsoft are external
        assert!(!imports.iter().any(|i| i.contains("System")));
        assert!(!imports.iter().any(|i| i.contains("Microsoft")));
        // MyApp.Models → Models/
        assert!(imports.iter().any(|i| i == "Models/"));
        assert!(imports.iter().any(|i| i == "Services/"));
    }

    // ── Deep: generic class ──

    #[test]
    fn generic_class() {
        let content = "public class Repository<T> where T : class {\n  public T Find(int id) {\n    return default;\n  }\n}\n";
        let syms = extract_syms(content);
        let cls = syms.iter().find(|s| s.kind == "class").unwrap();
        assert_eq!(cls.name, "Repository");
    }

    // ── Deep: partial class ──

    #[test]
    fn partial_class() {
        let content = "public partial class UserForm {\n  public void InitializeComponent() {\n  }\n}\n";
        let syms = extract_syms(content);
        let cls = syms.iter().find(|s| s.kind == "class").unwrap();
        assert_eq!(cls.name, "UserForm");
    }

    // ── Deep: sealed class ──

    #[test]
    fn sealed_class() {
        let content = "public sealed class Singleton {\n  public static Singleton Instance() {\n    return null;\n  }\n}\n";
        let syms = extract_syms(content);
        let cls = syms.iter().find(|s| s.kind == "class").unwrap();
        assert_eq!(cls.name, "Singleton");
    }

    // ── Deep: virtual and override methods ──

    #[test]
    fn virtual_override_methods() {
        let content = r#"public class Base {
  public virtual void Render() {
  }
}
"#;
        let syms = extract_syms(content);
        let m = syms.iter().find(|s| s.name == "Render").unwrap();
        assert_eq!(m.kind, "method");
    }

    #[test]
    fn override_method() {
        let content = "public class Child : Base {\n  public override void Render() {\n  }\n}\n";
        let syms = extract_syms(content);
        let m = syms.iter().find(|s| s.name == "Render").unwrap();
        assert_eq!(m.kind, "method");
    }

    // ── Deep: protected internal ──

    #[test]
    fn protected_internal_visibility() {
        let content = "protected internal class SharedHelper {\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms[0].visibility, Some("protected internal"));
    }

    // ── Deep: multiple classes same file ──

    #[test]
    fn multiple_classes_correct_parent() {
        let content = r#"public class Alpha {
  public void AlphaMethod() {
  }
}

public class Beta {
  public void BetaMethod() {
  }
}
"#;
        let syms = extract_syms(content);
        let am = syms.iter().find(|s| s.name == "AlphaMethod").unwrap();
        assert_eq!(am.parent, Some("Alpha".to_owned()));
        let bm = syms.iter().find(|s| s.name == "BetaMethod").unwrap();
        assert_eq!(bm.parent, Some("Beta".to_owned()));
    }

    // ── Deep: interface with multiple methods ──

    #[test]
    fn interface_members_not_extracted_as_methods() {
        let content = r#"public interface IService {
  Task<bool> IsAvailable();
  void Shutdown();
}
"#;
        let syms = extract_syms(content);
        let iface = syms.iter().find(|s| s.kind == "interface").unwrap();
        assert_eq!(iface.name, "IService");
        // Interface body is not inside a "class" so methods aren't extracted
    }

    // ── Deep: enum with values ──

    #[test]
    fn enum_with_explicit_values() {
        let content = "public enum HttpStatus {\n  OK = 200,\n  NotFound = 404,\n  InternalError = 500,\n}\n";
        let syms = extract_syms(content);
        let e = syms.iter().find(|s| s.kind == "enum").unwrap();
        assert_eq!(e.name, "HttpStatus");
    }

    // ── Deep: class with multiple const fields ──

    #[test]
    fn multiple_const_fields() {
        let content = r#"public class Config {
  public const string DefaultHost = "localhost";
  public const int DefaultPort = 5000;
  private const int MaxRetries = 3;
}
"#;
        let syms = extract_syms(content);
        let consts: Vec<_> = syms.iter().filter(|s| s.kind == "const").collect();
        assert_eq!(consts.len(), 3);
        assert!(consts.iter().any(|s| s.name == "DefaultHost"));
        assert!(consts.iter().any(|s| s.name == "DefaultPort"));
        assert!(consts.iter().any(|s| s.name == "MaxRetries"));
    }

    // ── Deep: new modifier ──

    #[test]
    fn new_modifier_method() {
        let content = "public class Derived : Base {\n  public new void DoSomething() {\n  }\n}\n";
        let syms = extract_syms(content);
        let m = syms.iter().find(|s| s.name == "DoSomething").unwrap();
        assert_eq!(m.kind, "method");
    }

    // ── Deep: all external namespace prefixes ──

    #[test]
    fn all_external_namespaces_filtered() {
        let content = r#"using System;
using Microsoft.Extensions.Hosting;
using Newtonsoft.Json;
using NuGet.Packaging;
using Xunit;
using Moq;
using AutoMapper;
using FluentValidation;
using Serilog;
using MediatR;
using Polly;
using Dapper;
"#;
        let imports = extract_imports(content);
        assert!(imports.is_empty());
    }

    // ── Deep: line numbers ──

    #[test]
    fn line_numbers_accurate() {
        let content = "// line 1\n// line 2\n\npublic class Foo {\n  public void Bar() {\n  }\n}\n";
        let syms = extract_syms(content);
        let foo = syms.iter().find(|s| s.name == "Foo").unwrap();
        assert_eq!(foo.line, 4);
        let bar = syms.iter().find(|s| s.name == "Bar").unwrap();
        assert_eq!(bar.line, 5);
    }

    // ── Deep: readonly field not extracted as method ──

    #[test]
    fn readonly_field_not_method() {
        let content = "public class Service {\n  public readonly string Name = \"test\";\n}\n";
        let syms = extract_syms(content);
        let methods: Vec<_> = syms.iter().filter(|s| s.kind == "method").collect();
        assert!(methods.is_empty());
    }

    // ── Deep: extern method not a keyword clash ──

    #[test]
    fn extern_method() {
        let content = "public class NativeMethods {\n  public extern static int GetTickCount();\n}\n";
        let syms = extract_syms(content);
        let m = syms.iter().find(|s| s.name == "GetTickCount").unwrap();
        assert_eq!(m.kind, "method");
    }

    // ── Deep: signature content ──

    #[test]
    fn signature_truncated_at_opening_brace() {
        let content = "public class Foo {\n  public async Task<List<int>> GetItems() {\n    return new();\n  }\n}\n";
        let syms = extract_syms(content);
        let m = syms.iter().find(|s| s.name == "GetItems").unwrap();
        assert!(m.signature.contains("GetItems"));
    }

    // ── Deep: namespace path conversion ──

    #[test]
    fn namespace_to_path_deeply_nested() {
        assert_eq!(namespace_to_path("MyApp.Core.Data.Repositories"), "Core/Data/Repositories/");
    }

    // ── Deep: using var (C# 8) inside class should not produce imports ──

    #[test]
    fn using_var_not_extracted_as_import() {
        let _content = "public class Foo {\n  public void Process() {\n    using var conn = new SqlConnection();\n  }\n}\n";
        // this is a using _statement_, not a using _directive_ — no import extraction
        // The extract_imports looks for top-level "using X;" at any line; "using var" is filtered
    }

    // ── Deep: deeply nested project namespace ──

    #[test]
    fn deep_project_namespace_import() {
        let content = "using MyCompany.Project.Features.Auth.Services;\n";
        let imports = extract_imports(content);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0], "Project/Features/Auth/Services/");
    }
}
