/// Shared parsing primitives used by all language parsers.

/// Tracks `/* ... */` block comments across lines.
pub struct CommentTracker {
    in_block: bool,
}

impl CommentTracker {
    pub fn new() -> Self {
        Self { in_block: false }
    }

    /// Returns true if the entire line should be skipped (inside block comment
    /// or is a single-line comment). Also advances internal state when `/*`
    /// or `*/` delimiters are encountered.
    ///
    /// `line_comment_prefix` — e.g. `"//"` for C-style, `"#"` for Python.
    pub fn is_comment(&mut self, trimmed: &str, line_comment_prefix: &str) -> bool {
        if self.in_block {
            if let Some(pos) = trimmed.find("*/") {
                self.in_block = false;
                let rest = trimmed[pos + 2..].trim();
                return rest.is_empty() || rest.starts_with(line_comment_prefix);
            }
            return true;
        }

        if trimmed.starts_with("/*") {
            if trimmed.contains("*/") {
                return true;
            }
            self.in_block = true;
            return true;
        }

        if trimmed.starts_with(line_comment_prefix) || trimmed.starts_with('*') {
            return true;
        }

        false
    }
}

/// Find the line (1-indexed) where a brace-delimited block ends.
/// Scans from `start_idx` counting `{` and `}`. Returns 1-indexed end line.
pub fn find_brace_end(lines: &[&str], start_idx: usize) -> usize {
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

/// Find the line where a semicolon appears, starting from `start_idx`.
pub fn find_semicolon_or_same(lines: &[&str], start_idx: usize) -> usize {
    for (i, line) in lines[start_idx..].iter().enumerate() {
        if line.contains(';') {
            return start_idx + i + 1;
        }
    }
    start_idx + 1
}

/// Find end of a declaration that could end with either `;` or a `{}`-block.
pub fn find_semicolon_or_brace_end(lines: &[&str], start_idx: usize) -> usize {
    let first_line = lines[start_idx];
    if first_line.contains('{') {
        return find_brace_end(lines, start_idx);
    }
    find_semicolon_or_same(lines, start_idx)
}

/// Update a brace-depth counter by scanning chars in `trimmed`.
pub fn update_brace_depth(trimmed: &str, depth: &mut i32) {
    for c in trimmed.chars() {
        match c {
            '{' => *depth += 1,
            '}' => *depth -= 1,
            _ => {}
        }
    }
}

/// Truncate a declaration line at the opening `{` to produce a compact signature.
pub fn make_signature_brace(trimmed: &str) -> String {
    if let Some(brace_pos) = trimmed.find('{') {
        trimmed[..=brace_pos].trim().to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// Truncate a declaration line at `:` — used for Python `def foo(x):`.
#[allow(dead_code)]
pub fn make_signature_colon(trimmed: &str) -> String {
    if let Some(colon_pos) = trimmed.find(':') {
        trimmed[..=colon_pos].trim().to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// Try to extract a name from `<keyword> <name>...` where name ends at a
/// non-alphanumeric/non-underscore char.
#[allow(dead_code)]
pub fn try_extract_keyword_name(rest: &str, keyword: &str) -> Option<String> {
    let after = rest.strip_prefix(keyword)?;
    let name_end = after.find(|c: char| !c.is_alphanumeric() && c != '_')?;
    let name = &after[..name_end];
    if name.is_empty() { None } else { Some(name.to_owned()) }
}

pub fn extract_preceding_comment(lines: &[&str], symbol_line_idx: usize) -> Option<String> {
    if symbol_line_idx == 0 {
        return None;
    }

    let mut comment_lines: Vec<String> = Vec::new();
    let mut idx = symbol_line_idx - 1;
    let mut in_block_comment = false;

    loop {
        let trimmed = lines[idx].trim();

        if trimmed.is_empty() && !in_block_comment {
            break;
        }

        if !in_block_comment && (trimmed == "*/" || trimmed.ends_with("*/")) {
            in_block_comment = true;
            let content = trimmed.trim_end_matches("*/").trim_end();
            if !content.is_empty() && content != "*" {
                let content = content.trim_start_matches('*').trim();
                if !content.is_empty() {
                    comment_lines.push(content.to_owned());
                }
            }
        } else if in_block_comment {
            if trimmed.starts_with("/**") || trimmed.starts_with("/*") {
                let inner = trimmed.trim_start_matches("/**").trim_start_matches("/*").trim();
                if !inner.is_empty() {
                    comment_lines.push(inner.to_owned());
                }
                break;
            } else {
                let stripped = trimmed.trim_start_matches('*');
                let stripped = if stripped.starts_with(' ') { &stripped[1..] } else { stripped };
                if !stripped.is_empty() || !comment_lines.is_empty() {
                    comment_lines.push(stripped.to_owned());
                }
            }
        } else if trimmed.starts_with("///") || trimmed.starts_with("//!") {
            let stripped = trimmed.trim_start_matches("///")
                .trim_start_matches("//!");
            let stripped = if stripped.starts_with(' ') { &stripped[1..] } else { stripped };
            comment_lines.push(stripped.to_owned());
        } else if trimmed.starts_with("//") {
            let stripped = &trimmed[2..];
            let stripped = if stripped.starts_with(' ') { &stripped[1..] } else { stripped };
            comment_lines.push(stripped.to_owned());
        } else if trimmed.starts_with('#') && !trimmed.starts_with("#[") {
            let stripped = &trimmed[1..];
            let stripped = if stripped.starts_with(' ') { &stripped[1..] } else { stripped };
            comment_lines.push(stripped.to_owned());
        } else {
            break;
        }

        if idx == 0 {
            break;
        }
        idx -= 1;
    }

    if comment_lines.is_empty() {
        return None;
    }

    comment_lines.reverse();
    Some(comment_lines.join("\n"))
}

pub fn extract_docstring_after(lines: &[&str], symbol_line_idx: usize) -> Option<String> {
    let mut idx = symbol_line_idx + 1;
    if idx >= lines.len() {
        return None;
    }

    let trimmed = lines[idx].trim();

    let delimiter = if trimmed.starts_with("\"\"\"") {
        "\"\"\""
    } else if trimmed.starts_with("'''") {
        "'''"
    } else {
        return None;
    };

    let after_open = &trimmed[3..];
    if let Some(close_pos) = after_open.find(delimiter) {
        let content = after_open[..close_pos].trim();
        return if content.is_empty() { None } else { Some(content.to_owned()) };
    }

    let mut doc_lines: Vec<String> = Vec::new();
    let first_content = after_open.trim();
    if !first_content.is_empty() {
        doc_lines.push(first_content.to_owned());
    }

    idx += 1;
    while idx < lines.len() {
        let line = lines[idx].trim();
        if line.contains(delimiter) {
            let before_close = line.split(delimiter).next().unwrap_or("").trim();
            if !before_close.is_empty() {
                doc_lines.push(before_close.to_owned());
            }
            break;
        }
        doc_lines.push(line.to_owned());
        idx += 1;
    }

    if doc_lines.is_empty() {
        None
    } else {
        Some(doc_lines.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── find_brace_end ──

    #[test]
    fn brace_end_single_line() {
        let lines = vec!["fn foo() {}", "next line"];
        assert_eq!(find_brace_end(&lines, 0), 1);
    }

    #[test]
    fn brace_end_multi_line() {
        let lines = vec!["fn foo() {", "    body", "}"];
        assert_eq!(find_brace_end(&lines, 0), 3);
    }

    #[test]
    fn brace_end_nested() {
        let lines = vec!["fn foo() {", "  if true {", "    x", "  }", "}"];
        assert_eq!(find_brace_end(&lines, 0), 5);
    }

    #[test]
    fn brace_end_no_braces() {
        let lines = vec!["fn foo();"];
        assert_eq!(find_brace_end(&lines, 0), 1);
    }

    // ── find_semicolon_or_same ──

    #[test]
    fn semicolon_on_same_line() {
        let lines = vec!["type X = i32;"];
        assert_eq!(find_semicolon_or_same(&lines, 0), 1);
    }

    #[test]
    fn semicolon_on_next_line() {
        let lines = vec!["type X", "  = i32;"];
        assert_eq!(find_semicolon_or_same(&lines, 0), 2);
    }

    #[test]
    fn no_semicolon() {
        let lines = vec!["type X = i32"];
        assert_eq!(find_semicolon_or_same(&lines, 0), 1);
    }

    // ── find_semicolon_or_brace_end ──

    #[test]
    fn semicolon_or_brace_picks_brace() {
        let lines = vec!["struct Foo {", "  x: i32,", "}"];
        assert_eq!(find_semicolon_or_brace_end(&lines, 0), 3);
    }

    #[test]
    fn semicolon_or_brace_picks_semicolon() {
        let lines = vec!["type Foo = Bar;"];
        assert_eq!(find_semicolon_or_brace_end(&lines, 0), 1);
    }

    // ── update_brace_depth ──

    #[test]
    fn brace_depth_open() {
        let mut depth = 0;
        update_brace_depth("  {", &mut depth);
        assert_eq!(depth, 1);
    }

    #[test]
    fn brace_depth_close() {
        let mut depth = 1;
        update_brace_depth("  }", &mut depth);
        assert_eq!(depth, 0);
    }

    #[test]
    fn brace_depth_multiple() {
        let mut depth = 0;
        update_brace_depth("  { { } }", &mut depth);
        assert_eq!(depth, 0);
    }

    // ── make_signature_brace ──

    #[test]
    fn signature_with_brace() {
        assert_eq!(make_signature_brace("fn foo() {"), "fn foo() {");
    }

    #[test]
    fn signature_without_brace() {
        assert_eq!(make_signature_brace("type X = i32;"), "type X = i32;");
    }

    #[test]
    fn signature_trims_after_brace() {
        assert_eq!(make_signature_brace("pub fn bar(x: i32)   {  "), "pub fn bar(x: i32)   {");
    }

    // ── make_signature_colon ──

    #[test]
    fn signature_colon() {
        assert_eq!(make_signature_colon("def foo(x):"), "def foo(x):");
    }

    #[test]
    fn signature_no_colon() {
        assert_eq!(make_signature_colon("some text"), "some text");
    }

    // ── try_extract_keyword_name ──

    #[test]
    fn extract_keyword_name_struct() {
        assert_eq!(try_extract_keyword_name("struct Foo {", "struct "), Some("Foo".to_owned()));
    }

    #[test]
    fn extract_keyword_name_class() {
        assert_eq!(try_extract_keyword_name("class MyClass extends", "class "), Some("MyClass".to_owned()));
    }

    #[test]
    fn extract_keyword_name_empty() {
        assert_eq!(try_extract_keyword_name("struct {", "struct "), None);
    }

    // ── CommentTracker ──

    #[test]
    fn single_line_comment() {
        let mut ct = CommentTracker::new();
        assert!(ct.is_comment("// this is a comment", "//"));
    }

    #[test]
    fn not_a_comment() {
        let mut ct = CommentTracker::new();
        assert!(!ct.is_comment("fn foo() {}", "//"));
    }

    #[test]
    fn block_comment_single_line() {
        let mut ct = CommentTracker::new();
        assert!(ct.is_comment("/* block comment */", "//"));
        assert!(!ct.is_comment("fn foo() {}", "//"));
    }

    #[test]
    fn block_comment_multi_line() {
        let mut ct = CommentTracker::new();
        assert!(ct.is_comment("/* start of block", "//"));
        assert!(ct.is_comment("   still in block", "//"));
        assert!(ct.is_comment("   end of block */", "//"));
        assert!(!ct.is_comment("fn real_code() {}", "//"));
    }

    #[test]
    fn block_comment_star_prefixed_lines() {
        let mut ct = CommentTracker::new();
        assert!(ct.is_comment("* doc comment line", "//"));
    }

    #[test]
    fn hash_comment_for_python() {
        let mut ct = CommentTracker::new();
        assert!(ct.is_comment("# python comment", "#"));
    }

    #[test]
    fn nested_block_comments_tracking() {
        let mut ct = CommentTracker::new();
        assert!(ct.is_comment("/*", "//"));
        assert!(ct.is_comment("inner line", "//"));
        assert!(ct.is_comment("*/", "//"));
        assert!(!ct.is_comment("code after", "//"));
    }

    // ── extract_preceding_comment ──

    #[test]
    fn preceding_rust_doc_comment() {
        let lines = vec![
            "/// Processes an input file.",
            "/// Returns the result.",
            "pub fn process_file() {}",
        ];
        let result = extract_preceding_comment(&lines, 2);
        assert!(result.is_some());
        let c = result.unwrap();
        assert!(c.contains("Processes an input file."));
        assert!(c.contains("Returns the result."));
    }

    #[test]
    fn preceding_slash_slash_comment() {
        let lines = vec![
            "// Helper function for testing",
            "fn helper() {}",
        ];
        let result = extract_preceding_comment(&lines, 1);
        assert!(result.is_some());
        assert!(result.unwrap().contains("Helper function for testing"));
    }

    #[test]
    fn preceding_hash_comment() {
        let lines = vec![
            "# Compute the sum of two numbers.",
            "def add(a, b):",
        ];
        let result = extract_preceding_comment(&lines, 1);
        assert!(result.is_some());
        assert!(result.unwrap().contains("Compute the sum"));
    }

    #[test]
    fn preceding_no_comment() {
        let lines = vec![
            "let x = 42;",
            "fn foo() {}",
        ];
        let result = extract_preceding_comment(&lines, 1);
        assert!(result.is_none());
    }

    #[test]
    fn preceding_blank_line_stops() {
        let lines = vec![
            "/// Old comment",
            "",
            "fn foo() {}",
        ];
        let result = extract_preceding_comment(&lines, 2);
        assert!(result.is_none());
    }

    #[test]
    fn preceding_at_file_start() {
        let lines = vec![
            "/// First line comment.",
            "fn foo() {}",
        ];
        let result = extract_preceding_comment(&lines, 1);
        assert!(result.is_some());
        assert!(result.unwrap().contains("First line comment."));
    }

    #[test]
    fn preceding_first_line_no_comment() {
        let lines = vec!["fn foo() {}"];
        let result = extract_preceding_comment(&lines, 0);
        assert!(result.is_none());
    }

    #[test]
    fn preceding_star_prefixed_block() {
        let lines = vec![
            "/**",
            " * A block doc comment.",
            " * Second line.",
            " */",
            "function foo() {}",
        ];
        let result = extract_preceding_comment(&lines, 4);
        assert!(result.is_some());
        let c = result.unwrap();
        assert!(c.contains("Second line."));
    }

    #[test]
    fn preceding_rust_bang_comment() {
        let lines = vec![
            "//! Module-level doc comment.",
            "mod my_module;",
        ];
        let result = extract_preceding_comment(&lines, 1);
        assert!(result.is_some());
        assert!(result.unwrap().contains("Module-level doc comment."));
    }

    #[test]
    fn preceding_skips_rust_attribute() {
        let lines = vec![
            "/// A documented function.",
            "#[inline]",
            "fn foo() {}",
        ];
        let result = extract_preceding_comment(&lines, 2);
        assert!(result.is_none());
    }

    // ── extract_docstring_after ──

    #[test]
    fn docstring_single_line() {
        let lines = vec![
            "def foo():",
            "    \"\"\"A short docstring.\"\"\"",
            "    pass",
        ];
        let result = extract_docstring_after(&lines, 0);
        assert!(result.is_some());
        assert!(result.unwrap().contains("A short docstring."));
    }

    #[test]
    fn docstring_multi_line() {
        let lines = vec![
            "class Foo:",
            "    \"\"\"",
            "    A multi-line",
            "    docstring.",
            "    \"\"\"",
            "    pass",
        ];
        let result = extract_docstring_after(&lines, 0);
        assert!(result.is_some());
        let c = result.unwrap();
        assert!(c.contains("A multi-line"));
        assert!(c.contains("docstring."));
    }

    #[test]
    fn docstring_single_quotes() {
        let lines = vec![
            "def bar():",
            "    '''Single quote docstring.'''",
            "    pass",
        ];
        let result = extract_docstring_after(&lines, 0);
        assert!(result.is_some());
        assert!(result.unwrap().contains("Single quote docstring."));
    }

    #[test]
    fn no_docstring() {
        let lines = vec![
            "def baz():",
            "    return True",
        ];
        let result = extract_docstring_after(&lines, 0);
        assert!(result.is_none());
    }

    #[test]
    fn docstring_at_end_of_file() {
        let lines = vec!["def foo():"];
        let result = extract_docstring_after(&lines, 0);
        assert!(result.is_none());
    }
}
