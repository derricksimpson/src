use std::path::Path;
use super::{LangImports, LangSymbols, SymbolInfo};
use super::common::{self, CommentTracker};

pub struct KotlinImports;

impl LangImports for KotlinImports {
    fn extensions(&self) -> &[&str] {
        &["kt", "kts"]
    }

    fn extract_imports(&self, content: &str, _file_path: &Path) -> Vec<String> {
        let mut imports = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();

            if let Some(rest) = trimmed.strip_prefix("import ") {
                let path = rest.trim().trim_end_matches(';');
                if path.is_empty() || is_kotlin_stdlib(path) {
                    continue;
                }
                let file_path = kotlin_import_to_path(path);
                imports.push(file_path);
            }
        }

        imports
    }
}

impl LangSymbols for KotlinImports {
    fn extensions(&self) -> &[&str] {
        &["kt", "kts"]
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
            if trimmed.starts_with('@') {
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

            let (vis, rest) = extract_kotlin_visibility(trimmed);
            let rest_clean = strip_kotlin_modifiers(rest);

            if let Some(kind_name) = try_kotlin_class(rest_clean) {
                let (kind, name) = kind_name;
                let end_line = if trimmed.contains('{') {
                    common::find_brace_end(&all_lines, line_idx)
                } else {
                    line_num
                };
                symbols.push(SymbolInfo {
                    kind,
                    name: name.clone(),
                    line: line_num,
                    end_line,
                    visibility: vis,
                    parent: None,
                    signature: common::make_signature_brace(trimmed),
                });
                if trimmed.contains('{') {
                    current_class = Some(name);
                    in_class = true;
                    class_brace_depth = 0;
                    common::update_brace_depth(trimmed, &mut class_brace_depth);
                }
                continue;
            }

            if let Some(name) = try_kotlin_object(rest_clean) {
                let end_line = if trimmed.contains('{') {
                    common::find_brace_end(&all_lines, line_idx)
                } else {
                    line_num
                };
                symbols.push(SymbolInfo {
                    kind: "class",
                    name: name.clone(),
                    line: line_num,
                    end_line,
                    visibility: vis,
                    parent: current_class.clone(),
                    signature: common::make_signature_brace(trimmed),
                });
                continue;
            }

            if rest_clean.starts_with("fun ") || rest_clean.starts_with("fun<") {
                if let Some(name) = extract_kotlin_fun_name(rest_clean) {
                    let end_line = if trimmed.contains('{') {
                        common::find_brace_end(&all_lines, line_idx)
                    } else {
                        line_num
                    };
                    let kind = if in_class { "method" } else { "fn" };
                    symbols.push(SymbolInfo {
                        kind,
                        name,
                        line: line_num,
                        end_line,
                        visibility: vis,
                        parent: if in_class { current_class.clone() } else { None },
                        signature: common::make_signature_brace(trimmed),
                    });
                    continue;
                }
            }

            if in_class {
                if let Some((kind, name)) = try_kotlin_property(rest_clean) {
                    let end_line = common::find_semicolon_or_same(&all_lines, line_idx);
                    symbols.push(SymbolInfo {
                        kind,
                        name,
                        line: line_num,
                        end_line,
                        visibility: vis,
                        parent: current_class.clone(),
                        signature: trimmed.to_owned(),
                    });
                    continue;
                }
            }
        }

        symbols
    }
}

fn extract_kotlin_visibility(trimmed: &str) -> (Option<&'static str>, &str) {
    if let Some(rest) = trimmed.strip_prefix("public ") {
        (Some("pub"), rest)
    } else if let Some(rest) = trimmed.strip_prefix("private ") {
        (Some("private"), rest)
    } else if let Some(rest) = trimmed.strip_prefix("protected ") {
        (Some("protected"), rest)
    } else if let Some(rest) = trimmed.strip_prefix("internal ") {
        (Some("internal"), rest)
    } else {
        (None, trimmed)
    }
}

fn strip_kotlin_modifiers(rest: &str) -> &str {
    let mut s = rest;
    for modifier in &[
        "open ", "final ", "abstract ", "sealed ", "data ", "inner ",
        "override ", "inline ", "noinline ", "crossinline ", "external ",
        "operator ", "infix ", "suspend ", "tailrec ", "actual ", "expect ",
        "companion ", "lateinit ", "const ", "enum ",
    ] {
        while let Some(r) = s.strip_prefix(modifier) {
            s = r;
        }
    }
    s
}

fn try_kotlin_class(rest: &str) -> Option<(&'static str, String)> {
    let (keyword, kind) = if let Some(after) = rest.strip_prefix("class ") {
        (after, "class")
    } else if let Some(after) = rest.strip_prefix("interface ") {
        (after, "interface")
    } else {
        return None;
    };

    let name_end = keyword.find(|c: char| !c.is_alphanumeric() && c != '_')?;
    let name = &keyword[..name_end];
    if name.is_empty() { return None; }
    Some((kind, name.to_owned()))
}

fn try_kotlin_object(rest: &str) -> Option<String> {
    let after = rest.strip_prefix("object ")?;
    if after.starts_with(':') || after.starts_with('{') {
        return None;
    }
    let name_end = after.find(|c: char| !c.is_alphanumeric() && c != '_').unwrap_or(after.len());
    let name = &after[..name_end];
    if name.is_empty() { None } else { Some(name.to_owned()) }
}

fn extract_kotlin_fun_name(rest: &str) -> Option<String> {
    let after = rest.strip_prefix("fun")?;

    let after = if after.starts_with('<') {
        let close = after.find('>')?;
        &after[close + 1..]
    } else {
        after
    };

    let after = after.strip_prefix(' ').unwrap_or(after);

    if let Some(dot_pos) = after.find('.') {
        let paren_pos = after.find('(').unwrap_or(after.len());
        if dot_pos < paren_pos {
            let after_dot = &after[dot_pos + 1..];
            let name_end = after_dot.find(|c: char| !c.is_alphanumeric() && c != '_')
                .unwrap_or(after_dot.len());
            let name = &after_dot[..name_end];
            if name.is_empty() { return None; }
            return Some(name.to_owned());
        }
    }

    let name_end = after.find(|c: char| !c.is_alphanumeric() && c != '_')
        .unwrap_or(after.len());
    let name = &after[..name_end];
    if name.is_empty() { None } else { Some(name.to_owned()) }
}

fn try_kotlin_property(rest: &str) -> Option<(&'static str, String)> {
    let (after, kind) = if let Some(a) = rest.strip_prefix("val ") {
        (a, "const")
    } else if let Some(a) = rest.strip_prefix("var ") {
        (a, "const")
    } else {
        return None;
    };

    let name_end = after.find(|c: char| !c.is_alphanumeric() && c != '_')
        .unwrap_or(after.len());
    let name = &after[..name_end];
    if name.is_empty() { return None; }
    Some((kind, name.to_owned()))
}

fn is_kotlin_stdlib(path: &str) -> bool {
    path.starts_with("kotlin.")
        || path.starts_with("java.")
        || path.starts_with("javax.")
        || path.starts_with("kotlinx.coroutines.")
}

fn kotlin_import_to_path(import_path: &str) -> String {
    let clean = if let Some(pos) = import_path.find(" as ") {
        &import_path[..pos]
    } else {
        import_path
    };

    if clean.ends_with(".*") {
        let pkg = &clean[..clean.len() - 2];
        format!("{}/", pkg.replace('.', "/"))
    } else {
        let parts: Vec<&str> = clean.rsplitn(2, '.').collect();
        if parts.len() == 2 {
            format!("{}/{}.kt", parts[1].replace('.', "/"), parts[0])
        } else {
            format!("{}.kt", clean.replace('.', "/"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extract_imports(content: &str) -> Vec<String> {
        KotlinImports.extract_imports(content, Path::new("src/Main.kt"))
    }

    fn extract_syms(content: &str) -> Vec<SymbolInfo> {
        <KotlinImports as LangSymbols>::extract_symbols(&KotlinImports, content)
    }

    // ── Import Tests ──

    #[test]
    fn simple_import() {
        let imports = extract_imports("import com.example.MyClass");
        assert_eq!(imports.len(), 1);
        assert!(imports[0].contains("com/example/MyClass.kt"));
    }

    #[test]
    fn wildcard_import() {
        let imports = extract_imports("import com.example.models.*");
        assert_eq!(imports.len(), 1);
        assert!(imports[0].ends_with('/'));
    }

    #[test]
    fn alias_import() {
        let imports = extract_imports("import com.example.MyClass as Mc");
        assert_eq!(imports.len(), 1);
        assert!(imports[0].contains("com/example/MyClass.kt"));
    }

    #[test]
    fn kotlin_stdlib_filtered() {
        let imports = extract_imports("import kotlin.collections.List\nimport java.util.HashMap");
        assert!(imports.is_empty());
    }

    #[test]
    fn multiple_imports() {
        let content = "import com.example.Foo\nimport com.example.Bar\nimport kotlin.Int\n";
        let imports = extract_imports(content);
        assert_eq!(imports.len(), 2);
    }

    #[test]
    fn no_imports() {
        let imports = extract_imports("class Main { }");
        assert!(imports.is_empty());
    }

    // ── Symbol: class ──

    #[test]
    fn extracts_class() {
        let content = "class MyClass {\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].kind, "class");
        assert_eq!(syms[0].name, "MyClass");
    }

    #[test]
    fn extracts_data_class() {
        let content = "data class Point(val x: Int, val y: Int)\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].kind, "class");
        assert_eq!(syms[0].name, "Point");
    }

    #[test]
    fn extracts_sealed_class() {
        let content = "sealed class Result {\n    data class Success(val data: String) : Result()\n}\n";
        let syms = extract_syms(content);
        let outer = syms.iter().find(|s| s.name == "Result").unwrap();
        assert_eq!(outer.kind, "class");
    }

    #[test]
    fn extracts_enum_class() {
        let content = "enum class Color {\n    RED, GREEN, BLUE\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms[0].kind, "class");
        assert_eq!(syms[0].name, "Color");
    }

    #[test]
    fn extracts_interface() {
        let content = "interface Runnable {\n    fun run()\n}\n";
        let syms = extract_syms(content);
        let iface = syms.iter().find(|s| s.kind == "interface").unwrap();
        assert_eq!(iface.name, "Runnable");
    }

    // ── Symbol: object ──

    #[test]
    fn extracts_object() {
        let content = "object Singleton {\n    fun instance() {}\n}\n";
        let syms = extract_syms(content);
        let obj = syms.iter().find(|s| s.name == "Singleton").unwrap();
        assert_eq!(obj.kind, "class");
    }

    #[test]
    fn extracts_companion_object() {
        let content = "class MyClass {\n    companion object Factory {\n        fun create(): MyClass = MyClass()\n    }\n}\n";
        let syms = extract_syms(content);
        let comp = syms.iter().find(|s| s.name == "Factory").unwrap();
        assert_eq!(comp.kind, "class");
        assert_eq!(comp.parent, Some("MyClass".to_owned()));
    }

    // ── Symbol: functions ──

    #[test]
    fn extracts_top_level_fun() {
        let content = "fun main(args: Array<String>) {\n    println(\"hello\")\n}\n";
        let syms = extract_syms(content);
        let f = syms.iter().find(|s| s.name == "main").unwrap();
        assert_eq!(f.kind, "fn");
        assert_eq!(f.parent, None);
    }

    #[test]
    fn extracts_method_in_class() {
        let content = "class Service {\n    fun process(input: String) {\n        println(input)\n    }\n}\n";
        let syms = extract_syms(content);
        let method = syms.iter().find(|s| s.kind == "method").unwrap();
        assert_eq!(method.name, "process");
        assert_eq!(method.parent, Some("Service".to_owned()));
    }

    #[test]
    fn extracts_suspend_fun() {
        let content = "suspend fun fetchData(): String {\n    return \"data\"\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms[0].name, "fetchData");
        assert_eq!(syms[0].kind, "fn");
    }

    #[test]
    fn extracts_generic_fun() {
        let content = "fun<T> identity(value: T): T {\n    return value\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms[0].name, "identity");
    }

    #[test]
    fn extracts_extension_fun() {
        let content = "fun String.addExclamation(): String {\n    return this + \"!\"\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms[0].name, "addExclamation");
    }

    #[test]
    fn expression_body_fun() {
        let content = "fun double(x: Int): Int = x * 2\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "double");
    }

    // ── Symbol: properties ──

    #[test]
    fn extracts_val_property_in_class() {
        let content = "class Config {\n    val MAX_SIZE = 1024\n}\n";
        let syms = extract_syms(content);
        let prop = syms.iter().find(|s| s.name == "MAX_SIZE").unwrap();
        assert_eq!(prop.kind, "const");
        assert_eq!(prop.parent, Some("Config".to_owned()));
    }

    #[test]
    fn extracts_var_property_in_class() {
        let content = "class State {\n    var count = 0\n}\n";
        let syms = extract_syms(content);
        let prop = syms.iter().find(|s| s.name == "count").unwrap();
        assert_eq!(prop.kind, "const");
    }

    // ── Visibility ──

    #[test]
    fn public_visibility() {
        let content = "public class Foo {\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms[0].visibility, Some("pub"));
    }

    #[test]
    fn private_visibility() {
        let content = "class Outer {\n    private fun helper() {\n    }\n}\n";
        let syms = extract_syms(content);
        let method = syms.iter().find(|s| s.kind == "method").unwrap();
        assert_eq!(method.visibility, Some("private"));
    }

    #[test]
    fn internal_visibility() {
        let content = "internal class InternalClass {\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms[0].visibility, Some("internal"));
    }

    #[test]
    fn default_visibility() {
        let content = "class DefaultVis {\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms[0].visibility, None);
    }

    // ── Comments ──

    #[test]
    fn skips_single_line_comments() {
        let content = "// class Commented {}\nclass Real {\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "Real");
    }

    #[test]
    fn skips_block_comments() {
        let content = "/* class Commented {} */\nclass Real {\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "Real");
    }

    #[test]
    fn skips_multiline_block_comments() {
        let content = "/**\n * class Commented {}\n */\nclass Real {\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "Real");
    }

    #[test]
    fn skips_annotations() {
        let content = "@JvmStatic\nfun main() {\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms[0].name, "main");
    }

    // ── Realistic ──

    #[test]
    fn realistic_kotlin_service() {
        let content = r#"package com.example.service

import com.example.model.User
import com.example.repository.UserRepository

class UserService(private val repo: UserRepository) {

    fun findAll(): List<User> {
        return repo.findAll()
    }

    fun findById(id: Long): User? {
        return repo.findById(id)
    }

    private fun validate(user: User): Boolean {
        return user.name.isNotBlank()
    }
}
"#;
        let syms = extract_syms(content);

        let cls = syms.iter().find(|s| s.kind == "class").unwrap();
        assert_eq!(cls.name, "UserService");

        let methods: Vec<_> = syms.iter().filter(|s| s.kind == "method").collect();
        assert_eq!(methods.len(), 3);
        assert!(methods.iter().any(|m| m.name == "findAll"));
        assert!(methods.iter().any(|m| m.name == "findById"));
        assert!(methods.iter().any(|m| m.name == "validate"));

        let private_method = methods.iter().find(|m| m.name == "validate").unwrap();
        assert_eq!(private_method.visibility, Some("private"));
    }

    #[test]
    fn realistic_kotlin_imports() {
        let content = "import com.example.model.User\nimport com.example.repository.UserRepository\nimport kotlin.collections.List\n";
        let imports = extract_imports(content);
        assert_eq!(imports.len(), 2);
        assert!(imports.iter().any(|i| i.contains("com/example/model/User.kt")));
        assert!(imports.iter().any(|i| i.contains("com/example/repository/UserRepository.kt")));
    }

    // ── Line numbers ──

    #[test]
    fn line_numbers_accurate() {
        let content = "package com.example\n\nimport kotlin.Int\n\nclass MyClass {\n    fun method1() {\n    }\n}\n";
        let syms = extract_syms(content);
        let cls = syms.iter().find(|s| s.kind == "class").unwrap();
        assert_eq!(cls.line, 5);
        let method = syms.iter().find(|s| s.kind == "method").unwrap();
        assert_eq!(method.line, 6);
    }

    // ── Edge cases ──

    #[test]
    fn class_with_generics() {
        let content = "class Container<T>(val value: T) {\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms[0].name, "Container");
    }

    #[test]
    fn class_with_inheritance() {
        let content = "class MyService : BaseService(), Serializable {\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms[0].name, "MyService");
    }

    // ── Helper function tests ──

    #[test]
    fn visibility_public() {
        let (vis, rest) = extract_kotlin_visibility("public class Foo {");
        assert_eq!(vis, Some("pub"));
        assert_eq!(rest, "class Foo {");
    }

    #[test]
    fn visibility_private() {
        let (vis, _) = extract_kotlin_visibility("private fun foo() {");
        assert_eq!(vis, Some("private"));
    }

    #[test]
    fn visibility_internal() {
        let (vis, _) = extract_kotlin_visibility("internal class Bar {");
        assert_eq!(vis, Some("internal"));
    }

    #[test]
    fn visibility_default() {
        let (vis, rest) = extract_kotlin_visibility("class Foo {");
        assert_eq!(vis, None);
        assert_eq!(rest, "class Foo {");
    }

    #[test]
    fn strip_modifiers() {
        assert_eq!(strip_kotlin_modifiers("open class Foo"), "class Foo");
        assert_eq!(strip_kotlin_modifiers("abstract fun bar()"), "fun bar()");
        assert_eq!(strip_kotlin_modifiers("data class Point"), "class Point");
        assert_eq!(strip_kotlin_modifiers("sealed class Result"), "class Result");
    }

    #[test]
    fn kotlin_stdlib_check() {
        assert!(is_kotlin_stdlib("kotlin.collections.List"));
        assert!(is_kotlin_stdlib("java.util.HashMap"));
        assert!(is_kotlin_stdlib("kotlinx.coroutines.launch"));
        assert!(!is_kotlin_stdlib("com.example.Foo"));
    }

    #[test]
    fn import_path_conversion() {
        assert_eq!(kotlin_import_to_path("com.example.MyClass"), "com/example/MyClass.kt");
        assert_eq!(kotlin_import_to_path("com.example.models.*"), "com/example/models/");
        assert_eq!(kotlin_import_to_path("com.example.MyClass as Mc"), "com/example/MyClass.kt");
    }
}
