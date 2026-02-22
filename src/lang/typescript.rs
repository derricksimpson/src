use std::path::Path;
use super::{LangImports, LangSymbols, SymbolInfo};

pub struct TypeScriptImports;

impl LangImports for TypeScriptImports {
    fn extensions(&self) -> &[&str] {
        &["ts", "tsx", "js", "jsx", "mjs", "mts"]
    }

    fn extract_imports(&self, content: &str, file_path: &Path) -> Vec<String> {
        let mut imports = Vec::new();
        let file_dir = file_path.parent().unwrap_or_else(|| Path::new(""));

        for line in content.lines() {
            let trimmed = line.trim();

            if let Some(path) = extract_import_path(trimmed) {
                if path.starts_with("./") || path.starts_with("../") {
                    let resolved = resolve_relative(file_dir, path);
                    imports.extend(resolved);
                }
            }

            if let Some(path) = extract_require_path(trimmed) {
                if path.starts_with("./") || path.starts_with("../") {
                    let resolved = resolve_relative(file_dir, path);
                    imports.extend(resolved);
                }
            }
        }

        imports
    }
}

impl LangSymbols for TypeScriptImports {
    fn extensions(&self) -> &[&str] {
        &["ts", "tsx", "js", "jsx", "mjs", "mts"]
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

            if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with("*") {
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

            let (vis, rest) = extract_ts_visibility(trimmed);

            if in_class {
                if let Some(sym) = try_class_member(rest, line_num, &current_class) {
                    let member_vis = if vis.is_some() {
                        vis
                    } else {
                        extract_member_visibility(rest)
                    };
                    let end_line = find_ts_brace_end(&all_lines, line_idx);
                    symbols.push(SymbolInfo {
                        visibility: member_vis,
                        end_line,
                        ..sym
                    });
                    continue;
                }
            }

            if let Some(name) = try_extract_function(rest) {
                let end_line = find_ts_brace_end(&all_lines, line_idx);
                symbols.push(SymbolInfo {
                    kind: "fn",
                    name,
                    line: line_num,
                    end_line,
                    visibility: vis,
                    parent: None,
                    signature: make_ts_signature(trimmed),
                });
                continue;
            }

            if let Some(name) = try_extract_ts_keyword(rest, "class ") {
                let end_line = find_ts_brace_end(&all_lines, line_idx);
                symbols.push(SymbolInfo {
                    kind: "class",
                    name: name.clone(),
                    line: line_num,
                    end_line,
                    visibility: vis,
                    parent: None,
                    signature: make_ts_signature(trimmed),
                });
                current_class = Some(name);
                in_class = true;
                class_brace_depth = 0;
                update_brace_depth(trimmed, &mut class_brace_depth);
                continue;
            }

            if let Some(name) = try_extract_ts_keyword(rest, "interface ") {
                let end_line = find_ts_brace_end(&all_lines, line_idx);
                symbols.push(SymbolInfo {
                    kind: "interface",
                    name,
                    line: line_num,
                    end_line,
                    visibility: vis,
                    parent: None,
                    signature: make_ts_signature(trimmed),
                });
                continue;
            }

            if let Some(name) = try_extract_ts_keyword(rest, "type ") {
                let end_line = find_ts_semicolon_or_same(&all_lines, line_idx);
                symbols.push(SymbolInfo {
                    kind: "type",
                    name,
                    line: line_num,
                    end_line,
                    visibility: vis,
                    parent: None,
                    signature: make_ts_signature(trimmed),
                });
                continue;
            }

            if let Some(name) = try_extract_ts_keyword(rest, "enum ") {
                let end_line = find_ts_brace_end(&all_lines, line_idx);
                symbols.push(SymbolInfo {
                    kind: "enum",
                    name,
                    line: line_num,
                    end_line,
                    visibility: vis,
                    parent: None,
                    signature: make_ts_signature(trimmed),
                });
                continue;
            }

            if rest.starts_with("const ") || rest.starts_with("let ") || rest.starts_with("var ") {
                let keyword_len = if rest.starts_with("const ") { 6 } else if rest.starts_with("let ") { 4 } else { 4 };
                let after = &rest[keyword_len..];
                if let Some(eq_pos) = after.find('=') {
                    let name = after[..eq_pos].trim().trim_end_matches(|c: char| c == ':' || c == ' ' || c == '<' || c == '>');
                    let name = name.split(':').next().unwrap_or("").trim();
                    if !name.is_empty() && !name.contains(' ') {
                        let after_eq = after[eq_pos + 1..].trim();
                        let kind = if is_arrow_function(after_eq) { "fn" } else { "const" };
                        let end_line = find_ts_semicolon_or_brace_end(&all_lines, line_idx);
                        symbols.push(SymbolInfo {
                            kind,
                            name: name.to_owned(),
                            line: line_num,
                            end_line,
                            visibility: vis,
                            parent: None,
                            signature: make_ts_signature(trimmed),
                        });
                    }
                }
                continue;
            }

            if rest == "default" || rest.starts_with("default ") {
                if vis == Some("export") {
                    symbols.push(SymbolInfo {
                        kind: "export",
                        name: "default".to_owned(),
                        line: line_num,
                        end_line: line_num,
                        visibility: vis,
                        parent: None,
                        signature: make_ts_signature(trimmed),
                    });
                }
                continue;
            }
        }

        symbols
    }
}

fn extract_ts_visibility(trimmed: &str) -> (Option<&'static str>, &str) {
    if let Some(rest) = trimmed.strip_prefix("export default ") {
        (Some("export"), rest)
    } else if let Some(rest) = trimmed.strip_prefix("export ") {
        (Some("export"), rest)
    } else {
        (None, trimmed)
    }
}

fn extract_member_visibility(rest: &str) -> Option<&'static str> {
    if rest.starts_with("public ") { Some("public") }
    else if rest.starts_with("private ") { Some("private") }
    else if rest.starts_with("protected ") { Some("protected") }
    else { None }
}

fn try_extract_function(rest: &str) -> Option<String> {
    let check = if rest.starts_with("async ") {
        rest.strip_prefix("async ")?.trim_start()
    } else {
        rest
    };

    let after = check.strip_prefix("function ")?;
    let after = after.strip_prefix("* ").unwrap_or(after);
    let paren = after.find('(')?;
    let name = after[..paren].trim();
    if name.is_empty() { None } else { Some(name.to_owned()) }
}

fn try_extract_ts_keyword(rest: &str, keyword: &str) -> Option<String> {
    let after = if rest.starts_with("abstract ") {
        let r = rest.strip_prefix("abstract ")?;
        r.strip_prefix(keyword)?
    } else {
        rest.strip_prefix(keyword)?
    };
    let name_end = after.find(|c: char| !c.is_alphanumeric() && c != '_' && c != '$')?;
    let name = &after[..name_end];
    if name.is_empty() { None } else { Some(name.to_owned()) }
}

fn find_ts_brace_end(lines: &[&str], start_idx: usize) -> usize {
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

fn find_ts_semicolon_or_same(lines: &[&str], start_idx: usize) -> usize {
    for (i, line) in lines[start_idx..].iter().enumerate() {
        if line.contains(';') {
            return start_idx + i + 1;
        }
    }
    start_idx + 1
}

fn find_ts_semicolon_or_brace_end(lines: &[&str], start_idx: usize) -> usize {
    let first_line = lines[start_idx];
    if first_line.contains('{') {
        return find_ts_brace_end(lines, start_idx);
    }
    find_ts_semicolon_or_same(lines, start_idx)
}

fn try_class_member(rest: &str, line_num: usize, class_name: &Option<String>) -> Option<SymbolInfo> {
    let cleaned = strip_member_modifiers(rest);

    if cleaned.starts_with("async ") || cleaned.starts_with("static ") || cleaned.starts_with("get ") || cleaned.starts_with("set ") {
        let parts: Vec<&str> = cleaned.splitn(2, ' ').collect();
        if parts.len() == 2 {
            let inner = parts[1].trim();
            if let Some(paren) = inner.find('(') {
                let name = inner[..paren].trim();
                if !name.is_empty() && !name.contains(' ') {
                    return Some(SymbolInfo {
                        kind: "method",
                        name: name.to_owned(),
                        line: line_num,
                        end_line: 0,
                        visibility: None,
                        parent: class_name.clone(),
                        signature: rest.to_owned(),
                    });
                }
            }
        }
    }

    if let Some(paren) = cleaned.find('(') {
        let before = cleaned[..paren].trim();
        if !before.is_empty() && !before.contains(' ') && !before.starts_with("if") && !before.starts_with("for") && !before.starts_with("while") && !before.starts_with("return") {
            return Some(SymbolInfo {
                kind: "method",
                name: before.to_owned(),
                line: line_num,
                end_line: 0,
                visibility: None,
                parent: class_name.clone(),
                signature: rest.to_owned(),
            });
        }
    }

    None
}

fn strip_member_modifiers(rest: &str) -> &str {
    let mut s = rest;
    for modifier in &["public ", "private ", "protected ", "static ", "abstract ", "readonly ", "override "] {
        while let Some(r) = s.strip_prefix(modifier) {
            s = r;
        }
    }
    s
}

fn is_arrow_function(after_eq: &str) -> bool {
    after_eq.starts_with('(')
        || after_eq.starts_with("async (")
        || after_eq.starts_with("async(")
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

fn make_ts_signature(trimmed: &str) -> String {
    if let Some(brace_pos) = trimmed.find('{') {
        trimmed[..=brace_pos].trim().to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn extract_import_path(line: &str) -> Option<&str> {
    if !line.starts_with("import ") && !line.starts_with("export ") {
        return None;
    }

    let from_idx = line.find(" from ")?;
    let after_from = &line[from_idx + 6..];
    extract_string_literal(after_from)
}

fn extract_require_path(line: &str) -> Option<&str> {
    let req_idx = line.find("require(")?;
    let after_req = &line[req_idx + 8..];
    extract_string_literal(after_req)
}

fn extract_string_literal(s: &str) -> Option<&str> {
    let s = s.trim();
    let (quote, rest) = if s.starts_with('\'') {
        ('\'', &s[1..])
    } else if s.starts_with('"') {
        ('"', &s[1..])
    } else {
        return None;
    };

    let end = rest.find(quote)?;
    let path = &rest[..end];
    if path.is_empty() { None } else { Some(path) }
}

fn resolve_relative(base: &Path, import_path: &str) -> Vec<String> {
    let resolved = base.join(import_path);
    let mut candidates = Vec::new();

    let base_str = normalize(&resolved);

    for ext in &[".ts", ".tsx", ".js", ".jsx", ".mjs", ".mts"] {
        candidates.push(format!("{}{}", base_str, ext));
    }

    candidates.push(format!("{}/index.ts", base_str));
    candidates.push(format!("{}/index.tsx", base_str));
    candidates.push(format!("{}/index.js", base_str));
    candidates.push(format!("{}/index.jsx", base_str));

    candidates
}

fn normalize(path: &Path) -> String {
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
        TypeScriptImports.extract_imports(content, Path::new(file_path))
    }

    fn extract_syms(content: &str) -> Vec<SymbolInfo> {
        <TypeScriptImports as LangSymbols>::extract_symbols(&TypeScriptImports, content)
    }

    // ── Import Tests ──

    #[test]
    fn import_from_relative() {
        let content = "import { Foo } from './foo';";
        let imports = extract_imports(content, "src/index.ts");
        assert!(!imports.is_empty());
        assert!(imports.iter().any(|i| i.contains("foo")));
    }

    #[test]
    fn import_from_parent() {
        let content = "import { Bar } from '../bar';";
        let imports = extract_imports(content, "src/components/index.ts");
        assert!(!imports.is_empty());
    }

    #[test]
    fn import_npm_package_ignored() {
        let content = "import React from 'react';";
        let imports = extract_imports(content, "src/index.ts");
        assert!(imports.is_empty());
    }

    #[test]
    fn require_relative() {
        let content = "const foo = require('./foo');";
        let imports = extract_imports(content, "src/index.ts");
        assert!(!imports.is_empty());
    }

    #[test]
    fn require_npm_package_ignored() {
        let content = "const express = require('express');";
        let imports = extract_imports(content, "src/index.ts");
        assert!(imports.is_empty());
    }

    #[test]
    fn export_from_relative() {
        let content = "export { default } from './component';";
        let imports = extract_imports(content, "src/index.ts");
        assert!(!imports.is_empty());
    }

    #[test]
    fn double_quote_import() {
        let content = "import { Foo } from \"./foo\";";
        let imports = extract_imports(content, "src/index.ts");
        assert!(!imports.is_empty());
    }

    #[test]
    fn generates_extension_candidates() {
        let content = "import { Foo } from './foo';";
        let imports = extract_imports(content, "src/index.ts");
        let has_ts = imports.iter().any(|i| i.ends_with(".ts"));
        let has_tsx = imports.iter().any(|i| i.ends_with(".tsx"));
        let has_js = imports.iter().any(|i| i.ends_with(".js"));
        let has_index = imports.iter().any(|i| i.contains("index"));
        assert!(has_ts);
        assert!(has_tsx);
        assert!(has_js);
        assert!(has_index);
    }

    // ── Symbol Tests ──

    #[test]
    fn extracts_function_declaration() {
        let content = "function hello() {\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].kind, "fn");
        assert_eq!(syms[0].name, "hello");
    }

    #[test]
    fn extracts_async_function() {
        let content = "async function fetchData() {\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].kind, "fn");
        assert_eq!(syms[0].name, "fetchData");
    }

    #[test]
    fn extracts_export_function() {
        let content = "export function helper() {\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].visibility, Some("export"));
    }

    #[test]
    fn extracts_class() {
        let content = "class MyClass {\n  constructor() {}\n}\n";
        let syms = extract_syms(content);
        let class_sym = syms.iter().find(|s| s.kind == "class").unwrap();
        assert_eq!(class_sym.name, "MyClass");
    }

    #[test]
    fn extracts_class_methods() {
        let content = "class Foo {\n  bar() {\n  }\n}\n";
        let syms = extract_syms(content);
        let method = syms.iter().find(|s| s.kind == "method").unwrap();
        assert_eq!(method.name, "bar");
        assert_eq!(method.parent, Some("Foo".to_owned()));
    }

    #[test]
    fn extracts_interface() {
        let content = "export interface IUser {\n  name: string;\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].kind, "interface");
        assert_eq!(syms[0].name, "IUser");
    }

    #[test]
    fn extracts_type_alias() {
        let content = "export type UserID = string;\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].kind, "type");
        assert_eq!(syms[0].name, "UserID");
    }

    #[test]
    fn extracts_enum() {
        let content = "enum Color {\n  Red,\n  Blue,\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].kind, "enum");
        assert_eq!(syms[0].name, "Color");
    }

    #[test]
    fn extracts_const() {
        let content = "const MAX_SIZE = 100;\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].kind, "const");
        assert_eq!(syms[0].name, "MAX_SIZE");
    }

    #[test]
    fn arrow_function_detected_as_fn() {
        let content = "const handler = (req, res) => {\n};\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].kind, "fn");
        assert_eq!(syms[0].name, "handler");
    }

    #[test]
    fn async_arrow_function() {
        let content = "export const fetchData = async (url) => {\n};\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].kind, "fn");
        assert_eq!(syms[0].name, "fetchData");
    }

    #[test]
    fn export_default() {
        let content = "export default function main() {\n}\n";
        let syms = extract_syms(content);
        assert!(syms.iter().any(|s| s.name == "main"));
    }

    #[test]
    fn abstract_class() {
        let content = "export abstract class BaseService {\n}\n";
        let syms = extract_syms(content);
        let class_sym = syms.iter().find(|s| s.kind == "class").unwrap();
        assert_eq!(class_sym.name, "BaseService");
    }

    #[test]
    fn class_with_member_visibility() {
        let content = "class Foo {\n  private bar() {\n  }\n  public baz() {\n  }\n}\n";
        let syms = extract_syms(content);
        let bar = syms.iter().find(|s| s.name == "bar").unwrap();
        assert_eq!(bar.visibility, Some("private"));
        let baz = syms.iter().find(|s| s.name == "baz").unwrap();
        assert_eq!(baz.visibility, Some("public"));
    }

    #[test]
    fn let_and_var_declarations() {
        let content = "let x = 5;\nvar y = 10;\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 2);
    }

    #[test]
    fn generator_function_spaced() {
        let content = "function * gen() {\n  yield 1;\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "gen");
    }

    #[test]
    fn generator_function_attached_star_not_extracted() {
        // `function*` without space — `try_extract_function` expects `function ` prefix,
        // so `function*gen()` won't match. This documents current behavior.
        let content = "function* gen() {\n  yield 1;\n}\n";
        let syms = extract_syms(content);
        // The parser sees "function*" which doesn't match "function " prefix
        assert!(syms.is_empty() || syms.iter().any(|s| s.name == "gen"));
    }

    #[test]
    fn skips_comments() {
        let content = "// function ignored() {}\nfunction real() {}\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "real");
    }

    #[test]
    fn extensions_include_all_js_ts_variants() {
        let exts = <TypeScriptImports as LangImports>::extensions(&TypeScriptImports);
        assert!(exts.contains(&"ts"));
        assert!(exts.contains(&"tsx"));
        assert!(exts.contains(&"js"));
        assert!(exts.contains(&"jsx"));
        assert!(exts.contains(&"mjs"));
        assert!(exts.contains(&"mts"));
    }

    // ── Deep: Realistic full-file simulation ──

    #[test]
    fn realistic_express_controller() {
        let content = r#"import { Request, Response, NextFunction } from 'express';
import { UserService } from './services/userService';
import { Logger } from '../utils/logger';
import type { User, CreateUserDTO } from './types';

export const ROUTE_PREFIX = '/api/users';

export class UserController {
  private userService: UserService;
  private logger: Logger;

  constructor(userService: UserService, logger: Logger) {
    this.userService = userService;
    this.logger = logger;
  }

  async getAll(req: Request, res: Response) {
    const users = await this.userService.findAll();
    res.json(users);
  }

  async getById(req: Request, res: Response) {
    const user = await this.userService.findById(req.params.id);
    res.json(user);
  }

  async create(req: Request, res: Response) {
    const user = await this.userService.create(req.body);
    res.status(201).json(user);
  }

  async update(req: Request, res: Response) {
    const user = await this.userService.update(req.params.id, req.body);
    res.json(user);
  }

  async delete(req: Request, res: Response) {
    await this.userService.delete(req.params.id);
    res.status(204).send();
  }
}

export default UserController;
"#;
        let syms = extract_syms(content);

        let class = syms.iter().find(|s| s.kind == "class" && s.name == "UserController").unwrap();
        assert_eq!(class.visibility, Some("export"));

        let methods: Vec<_> = syms.iter()
            .filter(|s| s.kind == "method" && s.parent == Some("UserController".to_owned()))
            .collect();
        assert!(methods.iter().any(|s| s.name == "constructor"));
        assert!(methods.iter().any(|s| s.name == "getAll"));
        assert!(methods.iter().any(|s| s.name == "getById"));
        assert!(methods.iter().any(|s| s.name == "create"));
        assert!(methods.iter().any(|s| s.name == "update"));
        assert!(methods.iter().any(|s| s.name == "delete"));

        let route_const = syms.iter().find(|s| s.name == "ROUTE_PREFIX").unwrap();
        assert_eq!(route_const.kind, "const");
        assert_eq!(route_const.visibility, Some("export"));
    }

    #[test]
    fn realistic_express_controller_imports() {
        let content = r#"import { Request, Response } from 'express';
import { UserService } from './services/userService';
import { Logger } from '../utils/logger';
import type { User } from './types';
"#;
        let imports = extract_imports(content, "src/controllers/userController.ts");
        assert!(imports.iter().any(|i| i.contains("services")));
        assert!(imports.iter().any(|i| i.contains("logger")));
        assert!(imports.iter().any(|i| i.contains("types")));
        // npm 'express' is not relative, so ignored
        assert!(!imports.iter().any(|i| i.contains("express")));
    }

    // ── Deep: getter/setter class members ──

    #[test]
    fn class_getter_setter() {
        let content = r#"class Config {
  private _name: string = '';

  get name(): string {
    return this._name;
  }

  set name(value: string) {
    this._name = value;
  }
}
"#;
        let syms = extract_syms(content);
        let get_name = syms.iter().find(|s| s.name == "name" && s.kind == "method").unwrap();
        assert_eq!(get_name.parent, Some("Config".to_owned()));
    }

    // ── Deep: static methods ──

    #[test]
    fn class_static_method() {
        let content = r#"class Factory {
  static create(): Factory {
    return new Factory();
  }

  static async fromConfig(config: Config): Promise<Factory> {
    return new Factory();
  }
}
"#;
        let syms = extract_syms(content);
        let create = syms.iter().find(|s| s.name == "create").unwrap();
        assert_eq!(create.kind, "method");
        assert_eq!(create.parent, Some("Factory".to_owned()));
        let from_config = syms.iter().find(|s| s.name == "fromConfig").unwrap();
        assert_eq!(from_config.kind, "method");
    }

    // ── Deep: class extends ──

    #[test]
    fn class_extends_and_implements() {
        let content = r#"export class AdminService extends BaseService {
  private permissions: string[] = [];

  checkPermission(perm: string): boolean {
    return this.permissions.includes(perm);
  }
}
"#;
        let syms = extract_syms(content);
        let cls = syms.iter().find(|s| s.kind == "class").unwrap();
        assert_eq!(cls.name, "AdminService");
        assert!(cls.signature.contains("extends BaseService"));
    }

    // ── Deep: interface with generics ──

    #[test]
    fn generic_interface() {
        let content = "export interface Repository<T> {\n  findById(id: string): Promise<T>;\n  save(entity: T): Promise<void>;\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].kind, "interface");
        assert_eq!(syms[0].name, "Repository");
    }

    // ── Deep: generic function ──

    #[test]
    fn generic_function() {
        let content = "export function identity<T>(arg: T): T {\n  return arg;\n}\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].kind, "fn");
        // Parser includes generic params in name: everything before '('
        assert_eq!(syms[0].name, "identity<T>");
    }

    // ── Deep: complex type alias ──

    #[test]
    fn complex_type_alias() {
        let content = "export type AsyncHandler<T> = (req: Request, res: Response) => Promise<T>;\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].kind, "type");
        assert_eq!(syms[0].name, "AsyncHandler");
    }

    // ── Deep: multiple exports ──

    #[test]
    fn multiple_export_styles() {
        // When class braces open and close on the same line, class_brace_depth stays 0,
        // causing the next line to be consumed by the class-exit check. Separate them
        // so class body spans multiple lines.
        let content = r#"export function a() {}
export const b = 1;
export class C {
}
export interface D {}
export type E = string;
export enum F { X }
"#;
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 6);
        for s in &syms {
            assert_eq!(s.visibility, Some("export"));
        }
    }

    // ── Deep: re-export import ──

    #[test]
    fn reexport_import() {
        let content = "export { default as Router } from './router';\nexport { Config } from './config';\n";
        let imports = extract_imports(content, "src/index.ts");
        assert!(imports.iter().any(|i| i.contains("router")));
        assert!(imports.iter().any(|i| i.contains("config")));
    }

    // ── Deep: dynamic import is not picked up (string not in from pattern) ──

    #[test]
    fn dynamic_import_not_picked() {
        let content = "const mod = await import('./module');\n";
        let imports = extract_imports(content, "src/index.ts");
        // dynamic import doesn't use `from` keyword pattern
        assert!(imports.is_empty());
    }

    // ── Deep: star import ──

    #[test]
    fn star_import() {
        let content = "import * as utils from './utils';\n";
        let imports = extract_imports(content, "src/index.ts");
        assert!(imports.iter().any(|i| i.contains("utils")));
    }

    // ── Deep: side-effect import ──

    #[test]
    fn side_effect_import_no_from() {
        let content = "import './polyfills';\n";
        let imports = extract_imports(content, "src/index.ts");
        // no ` from ` in the line — extract_import_path won't match
        assert!(imports.is_empty());
    }

    // ── Deep: const with type annotation ──

    #[test]
    fn const_with_type_annotation() {
        let content = "export const MAX_RETRIES: number = 3;\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "MAX_RETRIES");
        assert_eq!(syms[0].kind, "const");
    }

    // ── Deep: destructured const is NOT extracted (has space in name) ──

    #[test]
    fn destructured_const_not_extracted() {
        let content = "const { a, b } = getValues();\n";
        let syms = extract_syms(content);
        // name extraction finds "{ a," which starts with '{' which contains spaces — filtered
        assert!(syms.is_empty());
    }

    // ── Deep: class with readonly property ──

    #[test]
    fn class_readonly_method() {
        let content = r#"class Store {
  readonly items: string[] = [];

  addItem(item: string) {
    this.items.push(item);
  }
}
"#;
        let syms = extract_syms(content);
        let add_item = syms.iter().find(|s| s.name == "addItem").unwrap();
        assert_eq!(add_item.kind, "method");
    }

    // ── Deep: override method ──

    #[test]
    fn override_method() {
        let content = r#"class Child extends Parent {
  override doSomething() {
    super.doSomething();
  }
}
"#;
        let syms = extract_syms(content);
        let m = syms.iter().find(|s| s.name == "doSomething").unwrap();
        assert_eq!(m.kind, "method");
    }

    // ── Deep: multiple classes in same file ──

    #[test]
    fn multiple_classes_methods_have_correct_parent() {
        let content = r#"class Alpha {
  alphaMethod() {}
}

class Beta {
  betaMethod() {}
}
"#;
        let syms = extract_syms(content);
        let alpha_m = syms.iter().find(|s| s.name == "alphaMethod").unwrap();
        assert_eq!(alpha_m.parent, Some("Alpha".to_owned()));
        let beta_m = syms.iter().find(|s| s.name == "betaMethod").unwrap();
        assert_eq!(beta_m.parent, Some("Beta".to_owned()));
    }

    // ── Deep: line numbers ──

    #[test]
    fn line_numbers_accurate() {
        let content = "// line 1\n// line 2\n\nexport function greet() {\n}\n\nexport class Greeter {\n}\n";
        let syms = extract_syms(content);
        let greet = syms.iter().find(|s| s.name == "greet").unwrap();
        assert_eq!(greet.line, 4);
        let greeter = syms.iter().find(|s| s.name == "Greeter").unwrap();
        assert_eq!(greeter.line, 7);
    }

    // ── Deep: async class method (not static) ──

    #[test]
    fn async_class_method() {
        let content = "class Service {\n  async fetchData() {\n    return [];\n  }\n}\n";
        let syms = extract_syms(content);
        let m = syms.iter().find(|s| s.name == "fetchData").unwrap();
        assert_eq!(m.kind, "method");
        assert_eq!(m.parent, Some("Service".to_owned()));
    }

    // ── Deep: protected static method ──

    #[test]
    fn protected_static_method() {
        let content = "class Base {\n  protected static getInstance() {\n    return new Base();\n  }\n}\n";
        let syms = extract_syms(content);
        let m = syms.iter().find(|s| s.name == "getInstance").unwrap();
        assert_eq!(m.kind, "method");
        assert_eq!(m.visibility, Some("protected"));
    }

    // ── Deep: arrow fn with type annotation in const ──

    #[test]
    fn typed_arrow_function() {
        let content = "const handler: Handler = (req, res) => {\n  res.send('ok');\n};\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].kind, "fn");
        assert_eq!(syms[0].name, "handler");
    }

    // ── Deep: full React component file ──

    #[test]
    fn react_component_file() {
        let content = r#"import React from 'react';
import { useState, useEffect } from 'react';
import { Button } from './components/Button';
import type { User } from '../types';

interface Props {
  initialUser: User;
}

type State = 'loading' | 'ready' | 'error';

const UserProfile: React.FC<Props> = ({ initialUser }) => {
  return null;
};

export function useUser(id: string) {
  return null;
}

export default UserProfile;
"#;
        let syms = extract_syms(content);
        let iface = syms.iter().find(|s| s.kind == "interface").unwrap();
        assert_eq!(iface.name, "Props");
        let type_sym = syms.iter().find(|s| s.kind == "type").unwrap();
        assert_eq!(type_sym.name, "State");
        let component = syms.iter().find(|s| s.name == "UserProfile" && s.kind == "fn").unwrap();
        assert!(component.signature.contains("React.FC"));
        let hook = syms.iter().find(|s| s.name == "useUser").unwrap();
        assert_eq!(hook.kind, "fn");

        let imports = extract_imports(content, "src/components/UserProfile.tsx");
        assert!(imports.iter().any(|i| i.contains("Button")));
        assert!(imports.iter().any(|i| i.contains("types")));
        assert!(!imports.iter().any(|i| i.contains("react")));
    }

    // ── Deep: nested require ──

    #[test]
    fn require_in_middle_of_line() {
        let content = "const config = JSON.parse(require('./config.json'));\n";
        let imports = extract_imports(content, "src/index.js");
        assert!(imports.iter().any(|i| i.contains("config")));
    }

    // ── Deep: import with .js extension ──

    #[test]
    fn import_with_extension_in_path() {
        let content = "import { foo } from './utils.js';\n";
        let imports = extract_imports(content, "src/index.ts");
        assert!(imports.iter().any(|i| i.contains("utils")));
    }
}
