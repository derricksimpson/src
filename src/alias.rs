use std::path::Path;

use regex::Regex;

use crate::file_reader;

pub const ALIAS_PREFIX: &str = "alias:";

pub struct AliasMapping {
    pub prefix: String,
    pub targets: Vec<String>,
}

pub fn load_aliases(root: &Path) -> Vec<AliasMapping> {
    let mut aliases = parse_tsconfig(root);
    let vite = parse_vite_config(root);

    for va in vite {
        if !aliases.iter().any(|a| a.prefix == va.prefix) {
            aliases.push(va);
        }
    }

    aliases
}

pub fn resolve_alias(specifier: &str, aliases: &[AliasMapping]) -> Vec<String> {
    let mut candidates = Vec::new();
    for mapping in aliases {
        if specifier.starts_with(&mapping.prefix) {
            let remainder = &specifier[mapping.prefix.len()..];
            for target in &mapping.targets {
                let base = format!("{}{}", target, remainder);
                candidates.extend(probe_extensions(&base));
            }
        }
    }
    candidates
}

pub fn is_npm_scoped_package(specifier: &str) -> bool {
    if !specifier.starts_with('@') {
        return false;
    }
    let after_at = &specifier[1..];
    let slash_pos = match after_at.find('/') {
        Some(p) => p,
        None => return false,
    };
    let scope = &after_at[..slash_pos];
    if scope.is_empty() {
        return false;
    }
    let first = scope.as_bytes()[0];
    if !first.is_ascii_lowercase() {
        return false;
    }
    scope.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'.')
}

pub fn is_potential_alias(path: &str) -> bool {
    if path.starts_with("@/") || path.starts_with("~/") {
        return true;
    }
    if path.starts_with('@') && !is_npm_scoped_package(path) && path.contains('/') {
        return true;
    }
    if path.starts_with('~') && path.len() > 1 {
        return true;
    }
    false
}

fn probe_extensions(base: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    for ext in &[".ts", ".tsx", ".js", ".jsx", ".mjs", ".mts"] {
        candidates.push(format!("{}{}", base, ext));
    }
    candidates.push(format!("{}/index.ts", base));
    candidates.push(format!("{}/index.tsx", base));
    candidates.push(format!("{}/index.js", base));
    candidates.push(format!("{}/index.jsx", base));
    candidates
}

// ── tsconfig.json parsing ──

fn parse_tsconfig(root: &Path) -> Vec<AliasMapping> {
    let tsconfig_path = root.join("tsconfig.json");
    let (base_url, mut aliases) = parse_tsconfig_file(&tsconfig_path);

    if aliases.is_empty() {
        for alt in &["tsconfig.app.json", "tsconfig.base.json"] {
            let alt_path = root.join(alt);
            let (alt_base, alt_aliases) = parse_tsconfig_file(&alt_path);
            if !alt_aliases.is_empty() {
                return finalize_tsconfig_aliases(alt_aliases, alt_base, root);
            }
            let _ = alt_base;
        }
    }

    let content = match read_text_file(&tsconfig_path) {
        Some(c) => c,
        None => return finalize_tsconfig_aliases(aliases, base_url, root),
    };

    if let Some(extends_path) = extract_extends(&content) {
        let parent_path = root.join(&extends_path);
        let (parent_base, parent_aliases) = parse_tsconfig_file(&parent_path);

        if aliases.is_empty() && !parent_aliases.is_empty() {
            aliases = parent_aliases;
        }
        let base_url = base_url.or(parent_base);
        return finalize_tsconfig_aliases(aliases, base_url, root);
    }

    finalize_tsconfig_aliases(aliases, base_url, root)
}

fn finalize_tsconfig_aliases(
    aliases: Vec<AliasMapping>,
    base_url: Option<String>,
    root: &Path,
) -> Vec<AliasMapping> {
    if aliases.is_empty() {
        return Vec::new();
    }

    let base_dir = match base_url {
        Some(ref bu) => {
            let p = root.join(bu);
            normalize_path(&p)
        }
        None => normalize_path(root),
    };

    aliases
        .into_iter()
        .map(|mut m| {
            m.targets = m.targets.into_iter().map(|t| {
                let cleaned = t.trim_start_matches("./");
                if base_dir.is_empty() || base_dir == "." {
                    cleaned.to_owned()
                } else {
                    format!("{}/{}", base_dir.trim_end_matches('/'), cleaned)
                }
            }).collect();
            m
        })
        .collect()
}

fn parse_tsconfig_file(path: &Path) -> (Option<String>, Vec<AliasMapping>) {
    let content = match read_text_file(path) {
        Some(c) => c,
        None => return (None, Vec::new()),
    };

    let stripped = strip_json_comments(&content);
    let base_url = extract_base_url(&stripped);
    let aliases = extract_paths(&stripped);

    (base_url, aliases)
}

fn strip_json_comments(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let mut in_string = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    let chars: Vec<char> = content.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if in_line_comment {
            if chars[i] == '\n' {
                in_line_comment = false;
                result.push('\n');
            }
            i += 1;
            continue;
        }
        if in_block_comment {
            if chars[i] == '*' && i + 1 < chars.len() && chars[i + 1] == '/' {
                in_block_comment = false;
                result.push(' ');
                i += 2;
            } else {
                if chars[i] == '\n' {
                    result.push('\n');
                }
                i += 1;
            }
            continue;
        }
        if in_string {
            result.push(chars[i]);
            if chars[i] == '\\' && i + 1 < chars.len() {
                result.push(chars[i + 1]);
                i += 2;
                continue;
            }
            if chars[i] == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if chars[i] == '"' {
            in_string = true;
            result.push(chars[i]);
            i += 1;
            continue;
        }
        if chars[i] == '/' && i + 1 < chars.len() {
            if chars[i + 1] == '/' {
                in_line_comment = true;
                i += 2;
                continue;
            }
            if chars[i + 1] == '*' {
                in_block_comment = true;
                i += 2;
                continue;
            }
        }
        result.push(chars[i]);
        i += 1;
    }

    result
}

fn extract_base_url(content: &str) -> Option<String> {
    let re = Regex::new(r#""baseUrl"\s*:\s*"([^"]*)""#).ok()?;
    re.captures(content).map(|c| c[1].to_owned())
}

fn extract_extends(content: &str) -> Option<String> {
    let re = Regex::new(r#""extends"\s*:\s*"([^"]*)""#).ok()?;
    re.captures(content).map(|c| c[1].to_owned())
}

fn extract_paths(content: &str) -> Vec<AliasMapping> {
    let paths_re = match Regex::new(r#""paths"\s*:\s*\{"#) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let start = match paths_re.find(content) {
        Some(m) => m.end() - 1,
        None => return Vec::new(),
    };

    let brace_content = match extract_brace_block(content, start) {
        Some(s) => s,
        None => return Vec::new(),
    };

    let entry_re = match Regex::new(r#""([^"]+)"\s*:\s*\[([^\]]*)\]"#) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let value_re = match Regex::new(r#""([^"]+)""#) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let mut aliases = Vec::new();
    for cap in entry_re.captures_iter(&brace_content) {
        let key = &cap[1];
        let values_str = &cap[2];

        let prefix = key.trim_end_matches('*');
        let targets: Vec<String> = value_re
            .captures_iter(values_str)
            .map(|vc| {
                let val = &vc[1];
                val.trim_end_matches('*').trim_start_matches("./").to_owned()
            })
            .collect();

        if !prefix.is_empty() && !targets.is_empty() {
            aliases.push(AliasMapping {
                prefix: prefix.to_owned(),
                targets,
            });
        }
    }

    aliases
}

fn extract_brace_block(content: &str, open_pos: usize) -> Option<String> {
    let bytes = content.as_bytes();
    if open_pos >= bytes.len() || bytes[open_pos] != b'{' {
        return None;
    }
    let mut depth = 0;
    let mut in_string = false;
    for (i, &b) in bytes[open_pos..].iter().enumerate() {
        if in_string {
            if b == b'\\' {
                continue;
            }
            if b == b'"' {
                in_string = false;
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(content[open_pos..open_pos + i + 1].to_owned());
                }
            }
            _ => {}
        }
    }
    None
}

// ── Vite config parsing ──

fn parse_vite_config(root: &Path) -> Vec<AliasMapping> {
    let candidates = ["vite.config.ts", "vite.config.js", "vite.config.mts", "vite.config.mjs"];
    for name in &candidates {
        let path = root.join(name);
        let content = match read_text_file(&path) {
            Some(c) => c,
            None => continue,
        };
        let mut aliases = extract_vite_object_aliases(&content);
        aliases.extend(extract_vite_array_aliases(&content));
        if !aliases.is_empty() {
            return aliases;
        }
    }
    Vec::new()
}

fn extract_vite_object_aliases(content: &str) -> Vec<AliasMapping> {
    let re = match Regex::new(
        r#"['"]([@~][^'"]*)['"]\s*:\s*(?:path\.resolve\s*\([^)]*['"]([\w/.]+)['"]\s*\)|['"]([\w/.]+)['"])"#
    ) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let mut aliases = Vec::new();
    for cap in re.captures_iter(content) {
        let key = &cap[1];
        let target = cap.get(2).or_else(|| cap.get(3)).map(|m| m.as_str());
        if let Some(raw_target) = target {
            let cleaned = normalize_vite_target(raw_target);
            let prefix = if key.ends_with('/') {
                key.to_owned()
            } else {
                format!("{}/", key)
            };
            aliases.push(AliasMapping {
                prefix,
                targets: vec![cleaned],
            });
        }
    }
    aliases
}

fn extract_vite_array_aliases(content: &str) -> Vec<AliasMapping> {
    let re = match Regex::new(
        r#"find\s*:\s*['"]([@~][^'"]*)['"]\s*,\s*replacement\s*:\s*['"]([\w/.]+)['"]"#
    ) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let mut aliases = Vec::new();
    for cap in re.captures_iter(content) {
        let key = &cap[1];
        let raw_target = &cap[2];
        let cleaned = normalize_vite_target(raw_target);
        let prefix = if key.ends_with('/') {
            key.to_owned()
        } else {
            format!("{}/", key)
        };
        aliases.push(AliasMapping {
            prefix,
            targets: vec![cleaned],
        });
    }
    aliases
}

fn normalize_vite_target(raw: &str) -> String {
    let s = raw.trim_start_matches("./").trim_start_matches('/');
    if s.is_empty() {
        String::new()
    } else if s.ends_with('/') {
        s.to_owned()
    } else {
        format!("{}/", s)
    }
}

// ── Helpers ──

fn read_text_file(path: &Path) -> Option<String> {
    match file_reader::read_file(path) {
        Ok(Some(c)) => Some(c),
        _ => None,
    }
}

fn normalize_path(path: &Path) -> String {
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

    // ── is_npm_scoped_package ──

    #[test]
    fn npm_scope_angular() {
        assert!(is_npm_scoped_package("@angular/core"));
    }

    #[test]
    fn npm_scope_nestjs() {
        assert!(is_npm_scoped_package("@nestjs/common"));
    }

    #[test]
    fn npm_scope_types_node() {
        assert!(is_npm_scoped_package("@types/node"));
    }

    #[test]
    fn npm_scope_with_dots() {
        assert!(is_npm_scoped_package("@vue.js/compiler"));
    }

    #[test]
    fn not_npm_at_slash() {
        assert!(!is_npm_scoped_package("@/components/Foo"));
    }

    #[test]
    fn not_npm_at_upper() {
        assert!(!is_npm_scoped_package("@App/utils"));
    }

    #[test]
    fn not_npm_tilde() {
        assert!(!is_npm_scoped_package("~/utils"));
    }

    #[test]
    fn not_npm_bare() {
        assert!(!is_npm_scoped_package("react"));
    }

    #[test]
    fn not_npm_at_only() {
        assert!(!is_npm_scoped_package("@"));
    }

    // ── is_potential_alias ──

    #[test]
    fn alias_at_slash() {
        assert!(is_potential_alias("@/components/Foo"));
    }

    #[test]
    fn alias_tilde_slash() {
        assert!(is_potential_alias("~/utils/bar"));
    }

    #[test]
    fn alias_at_custom() {
        assert!(is_potential_alias("@App/utils"));
    }

    #[test]
    fn not_alias_npm_scoped() {
        assert!(!is_potential_alias("@angular/core"));
    }

    #[test]
    fn not_alias_bare_name() {
        assert!(!is_potential_alias("react"));
    }

    #[test]
    fn not_alias_bare_name_with_slash() {
        assert!(!is_potential_alias("lodash/fp"));
    }

    // ── probe_extensions ──

    #[test]
    fn probe_generates_all_candidates() {
        let candidates = probe_extensions("src/components/Button");
        assert!(candidates.contains(&"src/components/Button.ts".to_owned()));
        assert!(candidates.contains(&"src/components/Button.tsx".to_owned()));
        assert!(candidates.contains(&"src/components/Button.js".to_owned()));
        assert!(candidates.contains(&"src/components/Button.jsx".to_owned()));
        assert!(candidates.contains(&"src/components/Button.mjs".to_owned()));
        assert!(candidates.contains(&"src/components/Button.mts".to_owned()));
        assert!(candidates.contains(&"src/components/Button/index.ts".to_owned()));
        assert!(candidates.contains(&"src/components/Button/index.tsx".to_owned()));
        assert!(candidates.contains(&"src/components/Button/index.js".to_owned()));
        assert!(candidates.contains(&"src/components/Button/index.jsx".to_owned()));
    }

    // ── resolve_alias ──

    #[test]
    fn resolve_alias_basic() {
        let aliases = vec![AliasMapping {
            prefix: "@/".to_owned(),
            targets: vec!["src/".to_owned()],
        }];
        let candidates = resolve_alias("@/components/Button", &aliases);
        assert!(candidates.contains(&"src/components/Button.ts".to_owned()));
        assert!(candidates.contains(&"src/components/Button.tsx".to_owned()));
        assert!(candidates.contains(&"src/components/Button/index.ts".to_owned()));
    }

    #[test]
    fn resolve_alias_tilde() {
        let aliases = vec![AliasMapping {
            prefix: "~/".to_owned(),
            targets: vec!["lib/".to_owned()],
        }];
        let candidates = resolve_alias("~/api", &aliases);
        assert!(candidates.contains(&"lib/api.ts".to_owned()));
        assert!(candidates.contains(&"lib/api.js".to_owned()));
    }

    #[test]
    fn resolve_alias_no_match() {
        let aliases = vec![AliasMapping {
            prefix: "@/".to_owned(),
            targets: vec!["src/".to_owned()],
        }];
        let candidates = resolve_alias("~/something", &aliases);
        assert!(candidates.is_empty());
    }

    #[test]
    fn resolve_alias_multiple_targets() {
        let aliases = vec![AliasMapping {
            prefix: "@/".to_owned(),
            targets: vec!["src/".to_owned(), "generated/".to_owned()],
        }];
        let candidates = resolve_alias("@/models/User", &aliases);
        assert!(candidates.iter().any(|c| c.starts_with("src/models/")));
        assert!(candidates.iter().any(|c| c.starts_with("generated/models/")));
    }

    // ── strip_json_comments ──

    #[test]
    fn strip_line_comments() {
        let input = r#"{ // this is a comment
  "foo": "bar"
}"#;
        let stripped = strip_json_comments(input);
        assert!(stripped.contains(r#""foo""#));
        assert!(!stripped.contains("this is a comment"));
    }

    #[test]
    fn strip_block_comments() {
        let input = r#"{ /* block */
  "foo": "bar"
}"#;
        let stripped = strip_json_comments(input);
        assert!(stripped.contains(r#""foo""#));
        assert!(!stripped.contains("block"));
    }

    #[test]
    fn preserves_strings_with_slashes() {
        let input = r#"{ "url": "https://example.com" }"#;
        let stripped = strip_json_comments(input);
        assert!(stripped.contains("https://example.com"));
    }

    // ── extract_base_url ──

    #[test]
    fn base_url_extracted() {
        let content = r#"{ "compilerOptions": { "baseUrl": "." } }"#;
        assert_eq!(extract_base_url(content), Some(".".to_owned()));
    }

    #[test]
    fn base_url_src() {
        let content = r#"{ "compilerOptions": { "baseUrl": "./src" } }"#;
        assert_eq!(extract_base_url(content), Some("./src".to_owned()));
    }

    #[test]
    fn base_url_absent() {
        let content = r#"{ "compilerOptions": { } }"#;
        assert_eq!(extract_base_url(content), None);
    }

    // ── extract_paths ──

    #[test]
    fn extract_paths_basic() {
        let content = r#"{ "compilerOptions": { "paths": { "@/*": ["./src/*"] } } }"#;
        let aliases = extract_paths(content);
        assert_eq!(aliases.len(), 1);
        assert_eq!(aliases[0].prefix, "@/");
        assert_eq!(aliases[0].targets, vec!["src/"]);
    }

    #[test]
    fn extract_paths_multiple() {
        let content = r#"{ "compilerOptions": { "paths": { "@/*": ["./src/*"], "~/*": ["./lib/*"] } } }"#;
        let aliases = extract_paths(content);
        assert_eq!(aliases.len(), 2);
    }

    #[test]
    fn extract_paths_multiple_targets() {
        let content = r#"{ "compilerOptions": { "paths": { "@/*": ["./src/*", "./generated/*"] } } }"#;
        let aliases = extract_paths(content);
        assert_eq!(aliases.len(), 1);
        assert_eq!(aliases[0].targets.len(), 2);
    }

    #[test]
    fn extract_paths_none() {
        let content = r#"{ "compilerOptions": { } }"#;
        let aliases = extract_paths(content);
        assert!(aliases.is_empty());
    }

    // ── extract_extends ──

    #[test]
    fn extends_extracted() {
        let content = r#"{ "extends": "./tsconfig.base.json" }"#;
        assert_eq!(extract_extends(content), Some("./tsconfig.base.json".to_owned()));
    }

    #[test]
    fn extends_absent() {
        let content = r#"{ "compilerOptions": {} }"#;
        assert_eq!(extract_extends(content), None);
    }

    // ── extract_brace_block ──

    #[test]
    fn brace_block_simple() {
        let content = r#"{ "a": 1 }"#;
        assert_eq!(extract_brace_block(content, 0), Some(r#"{ "a": 1 }"#.to_owned()));
    }

    #[test]
    fn brace_block_nested() {
        let content = r#"{ "a": { "b": 1 } }"#;
        assert_eq!(extract_brace_block(content, 0), Some(content.to_owned()));
    }

    #[test]
    fn brace_block_not_at_brace() {
        assert_eq!(extract_brace_block("hello", 0), None);
    }

    // ── Vite object alias extraction ──

    #[test]
    fn vite_object_path_resolve() {
        let content = r#"
export default defineConfig({
  resolve: {
    alias: {
      '@': path.resolve(__dirname, 'src'),
    }
  }
});
"#;
        let aliases = extract_vite_object_aliases(content);
        assert_eq!(aliases.len(), 1);
        assert_eq!(aliases[0].prefix, "@/");
        assert_eq!(aliases[0].targets, vec!["src/"]);
    }

    #[test]
    fn vite_object_string_literal() {
        let content = r#"
export default {
  resolve: {
    alias: {
      '@': './src',
    }
  }
};
"#;
        let aliases = extract_vite_object_aliases(content);
        assert_eq!(aliases.len(), 1);
        assert_eq!(aliases[0].prefix, "@/");
        assert_eq!(aliases[0].targets, vec!["src/"]);
    }

    // ── Vite array alias extraction ──

    #[test]
    fn vite_array_form() {
        let content = r#"
export default {
  resolve: {
    alias: [
      { find: '@', replacement: './src' },
    ]
  }
};
"#;
        let aliases = extract_vite_array_aliases(content);
        assert_eq!(aliases.len(), 1);
        assert_eq!(aliases[0].prefix, "@/");
        assert_eq!(aliases[0].targets, vec!["src/"]);
    }

    // ── normalize_vite_target ──

    #[test]
    fn normalize_target_dot_slash() {
        assert_eq!(normalize_vite_target("./src"), "src/");
    }

    #[test]
    fn normalize_target_slash_prefix() {
        assert_eq!(normalize_vite_target("/src"), "src/");
    }

    #[test]
    fn normalize_target_plain() {
        assert_eq!(normalize_vite_target("src"), "src/");
    }

    #[test]
    fn normalize_target_trailing_slash() {
        assert_eq!(normalize_vite_target("src/"), "src/");
    }
}
