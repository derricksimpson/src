use std::path::Path;
use super::{LangImports, LangSymbols, SymbolInfo};
use super::common::CommentTracker;

pub struct RubyImports;

impl LangImports for RubyImports {
    fn extensions(&self) -> &[&str] {
        &["rb", "rake"]
    }

    fn extract_imports(&self, content: &str, _file_path: &Path) -> Vec<String> {
        let mut imports = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();

            if let Some(path) = extract_require(trimmed, "require_relative ") {
                imports.push(normalize_ruby_path(&path, true));
            } else if let Some(path) = extract_require(trimmed, "require ") {
                if !is_ruby_stdlib(&path) {
                    imports.push(normalize_ruby_path(&path, false));
                }
            }
        }

        imports
    }
}

impl LangSymbols for RubyImports {
    fn extensions(&self) -> &[&str] {
        &["rb", "rake"]
    }

    fn extract_symbols(&self, content: &str) -> Vec<SymbolInfo> {
        let all_lines: Vec<&str> = content.lines().collect();
        let mut symbols = Vec::new();
        let mut scope_stack: Vec<(String, &'static str)> = Vec::new();
        let mut comment_tracker = CommentTracker::new();
        let mut in_ruby_block_comment = false;

        for (line_idx, line) in all_lines.iter().enumerate() {
            let trimmed = line.trim();
            let line_num = line_idx + 1;

            if in_ruby_block_comment {
                if trimmed == "=end" {
                    in_ruby_block_comment = false;
                }
                continue;
            }
            if trimmed == "=begin" {
                in_ruby_block_comment = true;
                continue;
            }

            if trimmed.is_empty() || comment_tracker.is_comment(trimmed, "#") {
                continue;
            }

            if trimmed == "end" || trimmed.starts_with("end ") || trimmed.starts_with("end;") {
                scope_stack.pop();
                continue;
            }

            if trimmed.starts_with("require ") || trimmed.starts_with("require_relative ")
                || trimmed.starts_with("load ")
            {
                continue;
            }

            let (vis, rest) = extract_ruby_visibility(trimmed);

            if let Some(name) = rest.strip_prefix("module ") {
                let name = name.trim();
                let name_end = name.find(|c: char| !c.is_alphanumeric() && c != '_' && c != ':')
                    .unwrap_or(name.len());
                let name = &name[..name_end];
                if !name.is_empty() {
                    let end_line = find_ruby_end(&all_lines, line_idx);
                    let parent = scope_stack.last().map(|(n, _)| n.clone());
                    symbols.push(SymbolInfo {
                        kind: "module",
                        name: name.to_owned(),
                        line: line_num,
                        end_line,
                        visibility: vis,
                        parent,
                        signature: trimmed.to_owned(),
                    });
                    scope_stack.push((name.to_owned(), "module"));
                }
                continue;
            }

            if let Some(after) = rest.strip_prefix("class ") {
                let after = after.trim();
                if after.starts_with("<<") {
                    continue;
                }
                let name_end = after.find(|c: char| !c.is_alphanumeric() && c != '_' && c != ':')
                    .unwrap_or(after.len());
                let name = &after[..name_end];
                if !name.is_empty() {
                    let end_line = find_ruby_end(&all_lines, line_idx);
                    let parent = scope_stack.last().map(|(n, _)| n.clone());
                    symbols.push(SymbolInfo {
                        kind: "class",
                        name: name.to_owned(),
                        line: line_num,
                        end_line,
                        visibility: vis,
                        parent,
                        signature: trimmed.to_owned(),
                    });
                    scope_stack.push((name.to_owned(), "class"));
                }
                continue;
            }

            if let Some(name) = extract_def(rest) {
                let end_line = find_ruby_end(&all_lines, line_idx);
                let (kind, parent) = if let Some((class_name, _)) = scope_stack.last() {
                    ("method", Some(class_name.clone()))
                } else {
                    ("fn", None)
                };
                symbols.push(SymbolInfo {
                    kind,
                    name,
                    line: line_num,
                    end_line,
                    visibility: vis,
                    parent,
                    signature: make_ruby_signature(trimmed),
                });
                continue;
            }

            if let Some(name) = extract_constant(rest) {
                if scope_stack.iter().any(|(_, k)| *k == "class" || *k == "module") {
                    let parent = scope_stack.last().map(|(n, _)| n.clone());
                    symbols.push(SymbolInfo {
                        kind: "const",
                        name,
                        line: line_num,
                        end_line: line_num,
                        visibility: vis,
                        parent,
                        signature: trimmed.to_owned(),
                    });
                }
            }
        }

        symbols
    }
}

fn extract_require(trimmed: &str, prefix: &str) -> Option<String> {
    let rest = trimmed.strip_prefix(prefix)?;
    let rest = rest.trim();

    let (start_delim, end_delim) = if rest.starts_with('\'') {
        ('\'', '\'')
    } else if rest.starts_with('"') {
        ('"', '"')
    } else {
        return None;
    };

    let inner = &rest[1..];
    let end_pos = inner.find(end_delim)?;
    let path = &inner[..end_pos];

    if path.is_empty() {
        return None;
    }

    let _ = start_delim;
    Some(path.to_owned())
}

fn normalize_ruby_path(path: &str, is_relative: bool) -> String {
    let with_ext = if path.ends_with(".rb") {
        path.to_owned()
    } else {
        format!("{}.rb", path)
    };

    if is_relative {
        with_ext
    } else {
        with_ext
    }
}

fn extract_ruby_visibility(trimmed: &str) -> (Option<&'static str>, &str) {
    if let Some(rest) = trimmed.strip_prefix("private ") {
        if rest.starts_with("def ") || rest.starts_with("class ") {
            return (Some("private"), rest);
        }
    }
    if let Some(rest) = trimmed.strip_prefix("protected ") {
        if rest.starts_with("def ") || rest.starts_with("class ") {
            return (Some("protected"), rest);
        }
    }
    (None, trimmed)
}

fn extract_def(rest: &str) -> Option<String> {
    let after = rest.strip_prefix("def ")?;
    let after = after.trim();

    let after = if let Some(after_self) = after.strip_prefix("self.") {
        after_self
    } else {
        after
    };

    let name_end = after.find(|c: char| !c.is_alphanumeric() && c != '_' && c != '?' && c != '!')
        .unwrap_or(after.len());
    let name = &after[..name_end];

    if name.is_empty() { None } else { Some(name.to_owned()) }
}

fn extract_constant(rest: &str) -> Option<String> {
    let tokens: Vec<&str> = rest.splitn(2, '=').collect();
    if tokens.len() != 2 {
        return None;
    }
    let name = tokens[0].trim();

    if name.is_empty() || !name.chars().next()?.is_uppercase() {
        return None;
    }
    if !name.chars().all(|c| c.is_uppercase() || c == '_' || c.is_numeric()) {
        return None;
    }
    if name.len() < 2 {
        return None;
    }

    Some(name.to_owned())
}

fn find_ruby_end(lines: &[&str], start_idx: usize) -> usize {
    let mut depth: i32 = 0;
    let block_starters = [
        "def ", "class ", "module ", "do", "begin", "if ", "unless ",
        "case ", "while ", "until ", "for ",
    ];

    for (i, line) in lines[start_idx..].iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let trimmed_no_string = strip_ruby_strings(trimmed);

        for starter in &block_starters {
            if trimmed_no_string.starts_with(starter)
                || (starter == &"do" && (trimmed_no_string.ends_with(" do") || trimmed_no_string.ends_with(" do |")))
            {
                if !is_inline_end(&trimmed_no_string) {
                    depth += 1;
                }
                break;
            }
        }

        if trimmed_no_string == "end" || trimmed_no_string.starts_with("end ")
            || trimmed_no_string.starts_with("end;") || trimmed_no_string.starts_with("end)")
        {
            depth -= 1;
            if depth <= 0 {
                return start_idx + i + 1;
            }
        }
    }

    start_idx + 1
}

fn strip_ruby_strings(trimmed: &str) -> String {
    let mut result = String::with_capacity(trimmed.len());
    let mut in_single = false;
    let mut in_double = false;

    for c in trimmed.chars() {
        match c {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            _ if !in_single && !in_double => result.push(c),
            _ => {}
        }
    }

    result
}

fn is_inline_end(trimmed: &str) -> bool {
    let parts: Vec<&str> = trimmed.splitn(2, |c: char| c == ';' || c == '\n').collect();
    if parts.len() > 1 {
        let rest = parts[1].trim();
        if rest == "end" || rest.starts_with("end ") || rest.starts_with("end;") {
            return true;
        }
    }
    false
}

fn make_ruby_signature(trimmed: &str) -> String {
    trimmed.to_owned()
}

fn is_ruby_stdlib(path: &str) -> bool {
    const STDLIB: &[&str] = &[
        "json", "yaml", "csv", "net/http", "uri", "open-uri", "fileutils",
        "pathname", "tempfile", "stringio", "set", "ostruct", "optparse",
        "logger", "digest", "base64", "securerandom", "socket", "openssl",
        "erb", "cgi", "webrick", "benchmark", "pp", "io/console",
        "English", "abbrev", "bigdecimal", "date", "time", "timeout",
        "thread", "mutex_m", "monitor", "singleton", "forwardable",
        "delegate", "observer", "resolv", "shellwords", "tsort",
    ];
    STDLIB.contains(&path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extract_imports(content: &str) -> Vec<String> {
        RubyImports.extract_imports(content, Path::new("lib/main.rb"))
    }

    fn extract_syms(content: &str) -> Vec<SymbolInfo> {
        <RubyImports as LangSymbols>::extract_symbols(&RubyImports, content)
    }

    // ── Import Tests ──

    #[test]
    fn require_relative_single_quotes() {
        let imports = extract_imports("require_relative 'models/user'");
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0], "models/user.rb");
    }

    #[test]
    fn require_relative_double_quotes() {
        let imports = extract_imports("require_relative \"models/user\"");
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0], "models/user.rb");
    }

    #[test]
    fn require_relative_with_rb_extension() {
        let imports = extract_imports("require_relative 'models/user.rb'");
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0], "models/user.rb");
    }

    #[test]
    fn require_gem() {
        let imports = extract_imports("require 'rails'\nrequire 'sinatra'");
        assert_eq!(imports.len(), 2);
        assert_eq!(imports[0], "rails.rb");
        assert_eq!(imports[1], "sinatra.rb");
    }

    #[test]
    fn require_stdlib_filtered() {
        let imports = extract_imports("require 'json'\nrequire 'yaml'\nrequire 'csv'");
        assert!(imports.is_empty());
    }

    #[test]
    fn require_net_http_stdlib_filtered() {
        let imports = extract_imports("require 'net/http'");
        assert!(imports.is_empty());
    }

    #[test]
    fn multiple_imports_mixed() {
        let content = "require 'json'\nrequire_relative 'lib/helper'\nrequire 'nokogiri'\n";
        let imports = extract_imports(content);
        assert_eq!(imports.len(), 2);
        assert!(imports.iter().any(|i| i == "lib/helper.rb"));
        assert!(imports.iter().any(|i| i == "nokogiri.rb"));
    }

    #[test]
    fn no_imports() {
        let imports = extract_imports("class Foo\nend");
        assert!(imports.is_empty());
    }

    // ── Symbol: class ──

    #[test]
    fn extracts_class() {
        let content = "class MyClass\nend\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].kind, "class");
        assert_eq!(syms[0].name, "MyClass");
    }

    #[test]
    fn extracts_class_with_inheritance() {
        let content = "class Child < Parent\nend\n";
        let syms = extract_syms(content);
        assert_eq!(syms[0].kind, "class");
        assert_eq!(syms[0].name, "Child");
    }

    #[test]
    fn skips_eigenclass() {
        let content = "class << self\n  def foo\n  end\nend\n";
        let syms = extract_syms(content);
        let classes: Vec<_> = syms.iter().filter(|s| s.kind == "class").collect();
        assert!(classes.is_empty());
    }

    // ── Symbol: module ──

    #[test]
    fn extracts_module() {
        let content = "module MyModule\nend\n";
        let syms = extract_syms(content);
        assert_eq!(syms[0].kind, "module");
        assert_eq!(syms[0].name, "MyModule");
    }

    #[test]
    fn nested_module_and_class() {
        let content = "module Outer\n  class Inner\n  end\nend\n";
        let syms = extract_syms(content);
        let module = syms.iter().find(|s| s.kind == "module").unwrap();
        assert_eq!(module.name, "Outer");
        let cls = syms.iter().find(|s| s.kind == "class").unwrap();
        assert_eq!(cls.name, "Inner");
        assert_eq!(cls.parent, Some("Outer".to_owned()));
    }

    // ── Symbol: methods ──

    #[test]
    fn extracts_instance_method() {
        let content = "class Foo\n  def bar\n  end\nend\n";
        let syms = extract_syms(content);
        let method = syms.iter().find(|s| s.kind == "method").unwrap();
        assert_eq!(method.name, "bar");
        assert_eq!(method.parent, Some("Foo".to_owned()));
    }

    #[test]
    fn extracts_class_method_self() {
        let content = "class Foo\n  def self.create\n  end\nend\n";
        let syms = extract_syms(content);
        let method = syms.iter().find(|s| s.kind == "method").unwrap();
        assert_eq!(method.name, "create");
    }

    #[test]
    fn extracts_method_with_args() {
        let content = "class Foo\n  def process(input, output)\n  end\nend\n";
        let syms = extract_syms(content);
        let method = syms.iter().find(|s| s.kind == "method").unwrap();
        assert_eq!(method.name, "process");
    }

    #[test]
    fn extracts_predicate_method() {
        let content = "class Foo\n  def valid?\n  end\nend\n";
        let syms = extract_syms(content);
        let method = syms.iter().find(|s| s.kind == "method").unwrap();
        assert_eq!(method.name, "valid?");
    }

    #[test]
    fn extracts_bang_method() {
        let content = "class Foo\n  def save!\n  end\nend\n";
        let syms = extract_syms(content);
        let method = syms.iter().find(|s| s.kind == "method").unwrap();
        assert_eq!(method.name, "save!");
    }

    #[test]
    fn extracts_top_level_method() {
        let content = "def helper\nend\n";
        let syms = extract_syms(content);
        assert_eq!(syms[0].kind, "fn");
        assert_eq!(syms[0].name, "helper");
        assert_eq!(syms[0].parent, None);
    }

    // ── Visibility ──

    #[test]
    fn private_method() {
        let content = "class Foo\n  private def secret\n  end\nend\n";
        let syms = extract_syms(content);
        let method = syms.iter().find(|s| s.kind == "method").unwrap();
        assert_eq!(method.visibility, Some("private"));
    }

    #[test]
    fn protected_method() {
        let content = "class Foo\n  protected def internal\n  end\nend\n";
        let syms = extract_syms(content);
        let method = syms.iter().find(|s| s.kind == "method").unwrap();
        assert_eq!(method.visibility, Some("protected"));
    }

    // ── Constants ──

    #[test]
    fn extracts_constant() {
        let content = "class Config\n  MAX_SIZE = 1024\nend\n";
        let syms = extract_syms(content);
        let c = syms.iter().find(|s| s.kind == "const").unwrap();
        assert_eq!(c.name, "MAX_SIZE");
        assert_eq!(c.parent, Some("Config".to_owned()));
    }

    #[test]
    fn constant_in_module() {
        let content = "module App\n  VERSION = '1.0'\nend\n";
        let syms = extract_syms(content);
        let c = syms.iter().find(|s| s.kind == "const").unwrap();
        assert_eq!(c.name, "VERSION");
        assert_eq!(c.parent, Some("App".to_owned()));
    }

    #[test]
    fn top_level_constant_not_extracted() {
        let content = "MAX_SIZE = 1024\n";
        let syms = extract_syms(content);
        let consts: Vec<_> = syms.iter().filter(|s| s.kind == "const").collect();
        assert!(consts.is_empty());
    }

    // ── Comments ──

    #[test]
    fn skips_single_line_comments() {
        let content = "# class Commented\nclass Real\nend\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "Real");
    }

    #[test]
    fn skips_multiline_block_comments() {
        let content = "=begin\nclass Commented\n=end\nclass Real\nend\n";
        let syms = extract_syms(content);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "Real");
    }

    // ── Line numbers ──

    #[test]
    fn line_numbers_accurate() {
        let content = "require 'json'\n\nmodule App\n  class Service\n    def process\n    end\n  end\nend\n";
        let syms = extract_syms(content);
        let module = syms.iter().find(|s| s.kind == "module").unwrap();
        assert_eq!(module.line, 3);
        let cls = syms.iter().find(|s| s.kind == "class").unwrap();
        assert_eq!(cls.line, 4);
        let method = syms.iter().find(|s| s.kind == "method").unwrap();
        assert_eq!(method.line, 5);
    }

    // ── Realistic ──

    #[test]
    fn realistic_rails_controller() {
        let content = r#"module Api
  module V1
    class UsersController < ApplicationController
      def index
        @users = User.all
        render json: @users
      end

      def show
        @user = User.find(params[:id])
        render json: @user
      end

      private def authenticate
        # auth logic
      end
    end
  end
end
"#;
        let syms = extract_syms(content);

        let modules: Vec<_> = syms.iter().filter(|s| s.kind == "module").collect();
        assert_eq!(modules.len(), 2);
        assert!(modules.iter().any(|m| m.name == "Api"));
        assert!(modules.iter().any(|m| m.name == "V1"));

        let cls = syms.iter().find(|s| s.kind == "class").unwrap();
        assert_eq!(cls.name, "UsersController");
        assert_eq!(cls.parent, Some("V1".to_owned()));

        let methods: Vec<_> = syms.iter().filter(|s| s.kind == "method").collect();
        assert_eq!(methods.len(), 3);
        assert!(methods.iter().any(|m| m.name == "index"));
        assert!(methods.iter().any(|m| m.name == "show"));
        assert!(methods.iter().any(|m| m.name == "authenticate"));

        let private_method = methods.iter().find(|m| m.name == "authenticate").unwrap();
        assert_eq!(private_method.visibility, Some("private"));
    }

    #[test]
    fn realistic_ruby_imports() {
        let content = "require_relative 'models/user'\nrequire_relative 'services/auth_service'\nrequire 'json'\nrequire 'nokogiri'\n";
        let imports = extract_imports(content);
        assert_eq!(imports.len(), 3);
        assert!(imports.iter().any(|i| i == "models/user.rb"));
        assert!(imports.iter().any(|i| i == "services/auth_service.rb"));
        assert!(imports.iter().any(|i| i == "nokogiri.rb"));
    }

    // ── Helper tests ──

    #[test]
    fn extract_require_single_quotes() {
        assert_eq!(extract_require("require 'foo'", "require "), Some("foo".to_owned()));
    }

    #[test]
    fn extract_require_double_quotes() {
        assert_eq!(extract_require("require \"foo\"", "require "), Some("foo".to_owned()));
    }

    #[test]
    fn extract_require_no_quotes() {
        assert_eq!(extract_require("require foo", "require "), None);
    }

    #[test]
    fn ruby_stdlib_check() {
        assert!(is_ruby_stdlib("json"));
        assert!(is_ruby_stdlib("yaml"));
        assert!(is_ruby_stdlib("net/http"));
        assert!(!is_ruby_stdlib("rails"));
        assert!(!is_ruby_stdlib("nokogiri"));
    }

    #[test]
    fn extract_def_simple() {
        assert_eq!(extract_def("def foo"), Some("foo".to_owned()));
    }

    #[test]
    fn extract_def_with_args() {
        assert_eq!(extract_def("def foo(x, y)"), Some("foo".to_owned()));
    }

    #[test]
    fn extract_def_self() {
        assert_eq!(extract_def("def self.create"), Some("create".to_owned()));
    }

    #[test]
    fn extract_def_predicate() {
        assert_eq!(extract_def("def valid?"), Some("valid?".to_owned()));
    }

    #[test]
    fn extract_def_bang() {
        assert_eq!(extract_def("def save!"), Some("save!".to_owned()));
    }

    #[test]
    fn extract_constant_simple() {
        assert_eq!(extract_constant("MAX_SIZE = 100"), Some("MAX_SIZE".to_owned()));
    }

    #[test]
    fn extract_constant_lowercase_skipped() {
        assert_eq!(extract_constant("count = 5"), None);
    }

    #[test]
    fn extract_constant_single_char_skipped() {
        assert_eq!(extract_constant("X = 5"), None);
    }
}
