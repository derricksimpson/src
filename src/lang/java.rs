use std::path::Path;
use super::{LangImports, LangSymbols, SymbolInfo};
use super::common::{self, CommentTracker};

pub struct JavaImports;

impl LangImports for JavaImports {
    fn extensions(&self) -> &[&str] {
        &["java"]
    }

    fn extract_imports(&self, content: &str, _file_path: &Path) -> Vec<String> {
        let mut imports = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();

            if let Some(rest) = trimmed.strip_prefix("import ") {
                let rest = if let Some(r) = rest.strip_prefix("static ") { r } else { rest };
                if let Some(path) = rest.strip_suffix(';') {
                    let path = path.trim();
                    if !path.is_empty() && !is_jdk_package(path) {
                        let file_path = java_import_to_path(path);
                        imports.push(file_path);
                    }
                }
            }
        }

        imports
    }
}

impl LangSymbols for JavaImports {
    fn extensions(&self) -> &[&str] {
        &["java"]
    }

    fn extract_symbols(&self, content: &str) -> Vec<SymbolInfo> {
        let all_lines: Vec<&str> = content.lines().collect();
        let mut symbols = Vec::new();
        let mut current_class: Option<String> = None;
        let mut class_brace_depth: i32 = 0;
        let mut in_class = false;
        let mut comment_tracker = CommentTracker::new();

        for (line_idx, line) in all_lines.iter().enumerate() {
            let trimmed = line.trim();
            let line_num = line_idx + 1;

            if trimmed.is_empty() || comment_tracker.is_comment(trimmed, "//") {
                if in_class {
                    common::update_brace_depth(trimmed, &mut class_brace_depth);
                    if class_brace_depth <= 0 {
                        current_class = None;
                        in_class = false;
                    }
                }
                continue;
            }

            if trimmed.starts_with("import ") || trimmed.starts_with("package ") {
                continue;
            }
            if trimmed.starts_with('@') && !trimmed.starts_with("@interface ") {
                continue;
            }

            if in_class {
                common::update_brace_depth(trimmed, &mut class_brace_depth);
                if class_brace_depth <= 0 {
                    current_class = None;
                    in_class = false;
                    continue;
                }
            }

            let (vis, rest) = extract_java_visibility(trimmed);
            let rest_clean = strip_java_modifiers(rest);

            if let Some(name) = try_java_keyword(rest_clean, "class ") {
                let end_line = common::find_brace_end(&all_lines, line_idx);
                symbols.push(SymbolInfo {
                    kind: "class",
                    name: name.clone(),
                    line: line_num,
                    end_line,
                    visibility: vis,
                    parent: None,
                    signature: common::make_signature_brace(trimmed),
                });
                current_class = Some(name);
                in_class = true;
                class_brace_depth = 0;
                common::update_brace_depth(trimmed, &mut class_brace_depth);
                continue;
            }

            if let Some(name) = try_java_keyword(rest_clean, "interface ") {
                let end_line = common::find_brace_end(&all_lines, line_idx);
                symbols.push(SymbolInfo {
                    kind: "interface",
                    name,
                    line: line_num,
                    end_line,
                    visibility: vis,
                    parent: None,
                    signature: common::make_signature_brace(trimmed),
                });
                continue;
            }

            if let Some(name) = try_java_keyword(rest_clean, "enum ") {
                let end_line = common::find_brace_end(&all_lines, line_idx);
                symbols.push(SymbolInfo {
                    kind: "enum",
                    name,
                    line: line_num,
                    end_line,
                    visibility: vis,
                    parent: None,
                    signature: common::make_signature_brace(trimmed),
                });
                continue;
            }

            if let Some(name) = try_java_keyword(rest_clean, "@interface ") {
                let end_line = common::find_brace_end(&all_lines, line_idx);
                symbols.push(SymbolInfo {
                    kind: "interface",
                    name,
                    line: line_num,
                    end_line,
                    visibility: vis,
                    parent: None,
                    signature: common::make_signature_brace(trimmed),
                });
                continue;
            }

            if let Some(name) = try_java_keyword(rest_clean, "record ") {
                let end_line = common::find_brace_end(&all_lines, line_idx);
                symbols.push(SymbolInfo {
                    kind: "class",
                    name,
                    line: line_num,
                    end_line,
                    visibility: vis,
                    parent: None,
                    signature: common::make_signature_brace(trimmed),
                });
                continue;
            }

            if in_class {
                if let Some(name) = try_java_method(rest_clean)
                    .or_else(|| try_java_constructor(rest_clean, current_class.as_deref()))
                {
                    let end_line = common::find_brace_end(&all_lines, line_idx);
                    symbols.push(SymbolInfo {
                        kind: "method",
                        name,
                        line: line_num,
                        end_line,
                        visibility: vis,
                        parent: current_class.clone(),
                        signature: common::make_signature_brace(trimmed),
                    });
                    continue;
                }

                if let Some(name) = try_java_const(rest) {
                    let end_line = common::find_semicolon_or_same(&all_lines, line_idx);
                    symbols.push(SymbolInfo {
                        kind: "const",
                        name,
                        line: line_num,
                        end_line,
                        visibility: vis,
                        parent: current_class.clone(),
                        signature: trimmed.to_owned(),
                    });
                    continue;
                }
            } else if trimmed.contains('{') {
                if let Some(name) = try_java_method(rest_clean) {
                    let end_line = common::find_brace_end(&all_lines, line_idx);
                    symbols.push(SymbolInfo {
                        kind: "fn",
                        name,
                        line: line_num,
                        end_line,
                        visibility: vis,
                        parent: None,
                        signature: common::make_signature_brace(trimmed),
                    });
                }
            }
        }

        symbols
    }
}

fn extract_java_visibility(trimmed: &str) -> (Option<&'static str>, &str) {
    if let Some(rest) = trimmed.strip_prefix("public ") {
        (Some("pub"), rest)
    } else if let Some(rest) = trimmed.strip_prefix("protected ") {
        (Some("protected"), rest)
    } else if let Some(rest) = trimmed.strip_prefix("private ") {
        (Some("private"), rest)
    } else {
        (None, trimmed)
    }
}

fn strip_java_modifiers(rest: &str) -> &str {
    let mut s = rest;
    for modifier in &["static ", "final ", "abstract ", "synchronized ", "native ", "strictfp ",
                       "transient ", "volatile ", "sealed ", "non-sealed ", "default "] {
        while let Some(r) = s.strip_prefix(modifier) {
            s = r;
        }
    }
    s
}

fn try_java_keyword(rest: &str, keyword: &str) -> Option<String> {
    let after = rest.strip_prefix(keyword)?;
    let name_end = after.find(|c: char| !c.is_alphanumeric() && c != '_')?;
    let name = &after[..name_end];
    if name.is_empty() { None } else { Some(name.to_owned()) }
}

fn try_java_method(rest: &str) -> Option<String> {
    if rest.starts_with("if ") || rest.starts_with("for ") || rest.starts_with("while ")
        || rest.starts_with("switch ") || rest.starts_with("return ") || rest.starts_with("throw ")
        || rest.starts_with("new ") || rest.starts_with("catch ")
    {
        return None;
    }

    let paren_pos = rest.find('(')?;
    let before_paren = rest[..paren_pos].trim();

    let tokens: Vec<&str> = before_paren.split_whitespace().collect();
    if tokens.len() < 2 {
        return None;
    }

    let name = *tokens.last()?;
    if name.is_empty() || name.contains('.') || name.contains('<') {
        return None;
    }

    let name_end = name.find(|c: char| !c.is_alphanumeric() && c != '_').unwrap_or(name.len());
    let clean_name = &name[..name_end];
    if clean_name.is_empty() { None } else { Some(clean_name.to_owned()) }
}

fn try_java_constructor(rest: &str, class_name: Option<&str>) -> Option<String> {
    let class_name = class_name?;
    let paren_pos = rest.find('(')?;
    let before_paren = rest[..paren_pos].trim();
    if before_paren == class_name {
        Some(class_name.to_owned())
    } else {
        None
    }
}

fn try_java_const(rest: &str) -> Option<String> {
    if !rest.starts_with("static ") && !rest.contains("final ") {
        return None;
    }
    let cleaned = strip_java_modifiers(rest);

    let tokens: Vec<&str> = cleaned.split_whitespace().collect();
    if tokens.len() < 2 {
        return None;
    }

    let name_candidate = tokens[1];
    let name = name_candidate.trim_end_matches(|c: char| c == '=' || c == ';' || c.is_whitespace());

    if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return None;
    }

    if name.chars().all(|c| c.is_uppercase() || c == '_' || c.is_numeric()) && name.len() > 1 {
        Some(name.to_owned())
    } else {
        None
    }
}

fn is_jdk_package(path: &str) -> bool {
    path.starts_with("java.")
        || path.starts_with("javax.")
        || path.starts_with("sun.")
        || path.starts_with("com.sun.")
        || path.starts_with("jdk.")
        || path.starts_with("org.w3c.")
        || path.starts_with("org.xml.")
        || path.starts_with("org.ietf.")
}

fn java_import_to_path(import_path: &str) -> String {
    if import_path.ends_with(".*") {
        let pkg = &import_path[..import_path.len() - 2];
        format!("{}/", pkg.replace('.', "/"))
    } else {
        let parts: Vec<&str> = import_path.rsplitn(2, '.').collect();
        if parts.len() == 2 {
            format!("{}/{}.java", parts[1].replace('.', "/"), parts[0])
        } else {
            format!("{}.java", import_path.replace('.', "/"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extract_imports(content: &str) -> Vec<String> {
        JavaImports.extract_imports(content, Path::new("src/Main.java"))
    }

    fn extract_syms(content: &str) -> Vec<SymbolInfo> {
        <JavaImports as LangSymbols>::extract_symbols(&JavaImports, content)
    }

    // ── Import Tests ──

    #[test]
    fn simple_import() {
        let imports = extract_imports("import com.example.MyClass;");
        assert_eq!(imports.len(), 1);
        assert!(imports[0].contains("com/example/MyClass.java"));
    }

    #[test]
    fn static_import() {
        let imports = extract_imports("import static com.example.Utils.helper;");
        assert_eq!(imports.len(), 1);
        assert!(imports[0].contains("com/example/Utils"));
    }

    #[test]
    fn wildcard_import() {
        let imports = extract_imports("import com.example.models.*;");
        assert_eq!(imports.len(), 1);
        assert!(imports[0].ends_with('/'));
        assert!(imports[0].contains("com/example/models/"));
    }

    #[test]
    fn jdk_import_filtered() {
        let imports = extract_imports("import java.util.List;\nimport javax.swing.JFrame;");
        assert!(imports.is_empty());
    }

    #[test]
    fn multiple_imports() {
        let content = "import com.example.Foo;\nimport com.example.Bar;\nimport java.util.Map;\n";
        let imports = extract_imports(content);
        assert_eq!(imports.len(), 2);
    }

    #[test]
    fn no_imports() {
        let imports = extract_imports("public class Main { }");
        assert!(imports.is_empty());
    }

    // ── Symbol Tests ──

    #[test]
    fn extracts_public_class() {
        let content = "public class MyClass {\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].kind, "class");
        assert_eq!(syms[0].name, "MyClass");
        assert_eq!(syms[0].visibility, Some("pub"));
    }

    #[test]
    fn extracts_interface() {
        let content = "public interface Runnable {\n    void run();\n}\n";
        let syms = extract_syms(content);
        let iface = syms.iter().find(|s| s.kind == "interface").unwrap();
        assert_eq!(iface.name, "Runnable");
    }

    #[test]
    fn extracts_enum() {
        let content = "public enum Color {\n    RED, GREEN, BLUE\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms[0].kind, "enum");
        assert_eq!(syms[0].name, "Color");
    }

    #[test]
    fn extracts_method_in_class() {
        let content = r#"public class Service {
    public void processData(String input) {
        System.out.println(input);
    }
}
"#;
        let syms = extract_syms(content);
        let method = syms.iter().find(|s| s.kind == "method").unwrap();
        assert_eq!(method.name, "processData");
        assert_eq!(method.parent, Some("Service".to_owned()));
    }

    #[test]
    fn extracts_private_method() {
        let content = r#"public class Helper {
    private String formatName(String name) {
        return name.trim();
    }
}
"#;
        let syms = extract_syms(content);
        let method = syms.iter().find(|s| s.kind == "method").unwrap();
        assert_eq!(method.name, "formatName");
        assert_eq!(method.visibility, Some("private"));
    }

    #[test]
    fn extracts_static_method() {
        let content = r#"public class Utils {
    public static int max(int a, int b) {
        return a > b ? a : b;
    }
}
"#;
        let syms = extract_syms(content);
        let method = syms.iter().find(|s| s.kind == "method").unwrap();
        assert_eq!(method.name, "max");
    }

    #[test]
    fn extracts_abstract_class_and_method() {
        let content = r#"public abstract class Shape {
    public abstract double area();
}
"#;
        let syms = extract_syms(content);
        let cls = syms.iter().find(|s| s.kind == "class").unwrap();
        assert_eq!(cls.name, "Shape");
    }

    #[test]
    fn extracts_record() {
        let content = "public record Point(int x, int y) {\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].kind, "class");
        assert_eq!(syms[0].name, "Point");
    }

    #[test]
    fn skips_single_line_comments() {
        let content = "// public class Commented {}\npublic class Real {\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "Real");
    }

    #[test]
    fn skips_block_comments() {
        let content = "/* public class Commented {} */\npublic class Real {\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "Real");
    }

    #[test]
    fn skips_multiline_block_comments() {
        let content = "/*\n * public class Commented {}\n */\npublic class Real {\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "Real");
    }

    #[test]
    fn skips_annotations() {
        let content = "@Override\npublic class MyClass {\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "MyClass");
    }

    #[test]
    fn multiple_methods_in_class() {
        let content = r#"public class Calculator {
    public int add(int a, int b) {
        return a + b;
    }
    public int subtract(int a, int b) {
        return a - b;
    }
    private int multiply(int a, int b) {
        return a * b;
    }
}
"#;
        let syms = extract_syms(content);
        let methods: Vec<_> = syms.iter().filter(|s| s.kind == "method").collect();
        assert_eq!(methods.len(), 3);
        assert!(methods.iter().any(|m| m.name == "add"));
        assert!(methods.iter().any(|m| m.name == "subtract"));
        assert!(methods.iter().any(|m| m.name == "multiply"));
        for m in &methods {
            assert_eq!(m.parent, Some("Calculator".to_owned()));
        }
    }

    #[test]
    fn class_with_extends_and_implements() {
        let content = "public class MyService extends BaseService implements Serializable {\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms[0].kind, "class");
        assert_eq!(syms[0].name, "MyService");
    }

    #[test]
    fn generic_class() {
        let content = "public class Container<T> {\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms[0].kind, "class");
        assert_eq!(syms[0].name, "Container");
    }

    #[test]
    fn const_field() {
        let content = r#"public class Config {
    public static final int MAX_SIZE = 1024;
    public static final String NAME = "test";
}
"#;
        let syms = extract_syms(content);
        let consts: Vec<_> = syms.iter().filter(|s| s.kind == "const").collect();
        assert_eq!(consts.len(), 2);
        assert!(consts.iter().any(|c| c.name == "MAX_SIZE"));
        assert!(consts.iter().any(|c| c.name == "NAME"));
    }

    #[test]
    fn protected_visibility() {
        let content = r#"public class Base {
    protected void init() {
    }
}
"#;
        let syms = extract_syms(content);
        let method = syms.iter().find(|s| s.kind == "method").unwrap();
        assert_eq!(method.visibility, Some("protected"));
    }

    #[test]
    fn package_private_visibility() {
        let content = r#"class Internal {
    void doWork() {
    }
}
"#;
        let syms = extract_syms(content);
        let cls = syms.iter().find(|s| s.kind == "class").unwrap();
        assert_eq!(cls.visibility, None);
    }

    #[test]
    fn line_numbers_accurate() {
        let content = r#"package com.example;

import java.util.List;

public class MyClass {
    public void method1() {
    }
}
"#;
        let syms = extract_syms(content);
        let cls = syms.iter().find(|s| s.kind == "class").unwrap();
        assert_eq!(cls.line, 5);
        let method = syms.iter().find(|s| s.kind == "method").unwrap();
        assert_eq!(method.line, 6);
    }

    #[test]
    fn annotation_interface() {
        let content = "public @interface MyAnnotation {\n    String value();\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].kind, "interface");
        assert_eq!(syms[0].name, "MyAnnotation");
    }

    #[test]
    fn realistic_spring_controller() {
        let content = r#"package com.example.controller;

import org.springframework.web.bind.annotation.RestController;
import org.springframework.web.bind.annotation.GetMapping;
import com.example.service.UserService;
import com.example.model.User;

@RestController
public class UserController {

    private final UserService userService;

    public UserController(UserService userService) {
        this.userService = userService;
    }

    @GetMapping("/users")
    public List<User> getUsers() {
        return userService.findAll();
    }

    @GetMapping("/users/{id}")
    public User getUser(Long id) {
        return userService.findById(id);
    }
}
"#;
        let syms = extract_syms(content);

        let cls = syms.iter().find(|s| s.kind == "class").unwrap();
        assert_eq!(cls.name, "UserController");

        let methods: Vec<_> = syms.iter().filter(|s| s.kind == "method").collect();
        assert!(methods.iter().any(|m| m.name == "UserController"));
        assert!(methods.iter().any(|m| m.name == "getUsers"));
        assert!(methods.iter().any(|m| m.name == "getUser"));
    }

    #[test]
    fn realistic_spring_controller_imports() {
        let content = r#"import com.example.service.UserService;
import com.example.model.User;
import org.springframework.web.bind.annotation.RestController;
import java.util.List;
"#;
        let imports = extract_imports(content);
        assert_eq!(imports.len(), 3);
        assert!(imports.iter().any(|i| i.contains("com/example/service/UserService.java")));
        assert!(imports.iter().any(|i| i.contains("com/example/model/User.java")));
        assert!(imports.iter().any(|i| i.contains("org/springframework")));
    }

    // ── Import edge cases ──

    #[test]
    fn import_to_path_simple() {
        assert_eq!(java_import_to_path("com.example.MyClass"), "com/example/MyClass.java");
    }

    #[test]
    fn import_to_path_wildcard() {
        assert_eq!(java_import_to_path("com.example.models.*"), "com/example/models/");
    }

    #[test]
    fn import_to_path_single_class() {
        assert_eq!(java_import_to_path("MyClass"), "MyClass.java");
    }

    #[test]
    fn is_jdk_java() {
        assert!(is_jdk_package("java.util.List"));
        assert!(is_jdk_package("javax.swing.JFrame"));
        assert!(!is_jdk_package("com.example.Foo"));
    }

    // ── Visibility extraction ──

    #[test]
    fn visibility_public() {
        let (vis, rest) = extract_java_visibility("public class Foo {");
        assert_eq!(vis, Some("pub"));
        assert_eq!(rest, "class Foo {");
    }

    #[test]
    fn visibility_private() {
        let (vis, _) = extract_java_visibility("private void foo() {");
        assert_eq!(vis, Some("private"));
    }

    #[test]
    fn visibility_protected() {
        let (vis, _) = extract_java_visibility("protected int bar;");
        assert_eq!(vis, Some("protected"));
    }

    #[test]
    fn visibility_default() {
        let (vis, rest) = extract_java_visibility("class Foo {");
        assert_eq!(vis, None);
        assert_eq!(rest, "class Foo {");
    }

    // ── Modifier stripping ──

    #[test]
    fn strip_static_final() {
        assert_eq!(strip_java_modifiers("static final int X = 5;"), "int X = 5;");
    }

    #[test]
    fn strip_abstract() {
        assert_eq!(strip_java_modifiers("abstract void foo();"), "void foo();");
    }

    #[test]
    fn strip_synchronized() {
        assert_eq!(strip_java_modifiers("synchronized void foo() {"), "void foo() {");
    }

    // ── Method extraction ──

    #[test]
    fn method_with_return_type() {
        assert_eq!(try_java_method("String getName()"), Some("getName".to_owned()));
    }

    #[test]
    fn method_void() {
        assert_eq!(try_java_method("void doWork()"), Some("doWork".to_owned()));
    }

    #[test]
    fn method_with_params() {
        assert_eq!(try_java_method("int add(int a, int b)"), Some("add".to_owned()));
    }

    #[test]
    fn method_skips_if() {
        assert_eq!(try_java_method("if (condition)"), None);
    }

    #[test]
    fn method_skips_for() {
        assert_eq!(try_java_method("for (int i = 0; i < 10; i++)"), None);
    }

    #[test]
    fn method_single_token_not_extracted() {
        assert_eq!(try_java_method("doSomething()"), None);
    }

    // ── Const extraction ──

    #[test]
    fn const_static_final() {
        assert_eq!(try_java_const("static final int MAX_SIZE = 100;"), Some("MAX_SIZE".to_owned()));
    }

    #[test]
    fn const_not_uppercase_skipped() {
        assert_eq!(try_java_const("static final int count = 5;"), None);
    }
}
