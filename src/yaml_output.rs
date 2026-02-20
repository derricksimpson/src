use std::io::{self, Write, BufWriter};

use crate::models::{
    CountEntry, FileChunk, FileEntry, GraphEntry, LangStats, LargestFile,
    MetaInfo, OutputEnvelope, ScanResult, StatsOutput, SymbolEntry, SymbolFile,
};

pub fn write_output(envelope: &OutputEnvelope) {
    let stdout = io::stdout();
    let mut w = BufWriter::with_capacity(64 * 1024, stdout.lock());
    write_envelope(&mut w, envelope).ok();
    w.flush().ok();
}

fn write_envelope(w: &mut impl Write, envelope: &OutputEnvelope) -> io::Result<()> {
    if let Some(ref meta) = envelope.meta {
        write_meta(w, meta)?;
    }
    if let Some(ref error) = envelope.error {
        write_scalar(w, "error", error, 0)?;
    }
    if let Some(ref tree) = envelope.tree {
        write!(w, "tree:\n")?;
        write_tree_node(w, tree, 2)?;
    }
    if let Some(ref graph) = envelope.graph {
        write_graph(w, graph)?;
    }
    if let Some(ref symbols) = envelope.symbols {
        write_symbols(w, symbols)?;
    }
    if let Some(ref counts) = envelope.counts {
        write_counts(w, counts)?;
    }
    if let Some(ref stats) = envelope.stats {
        write_stats(w, stats)?;
    }
    if let Some(ref files) = envelope.files {
        if !files.is_empty() {
            write_files(w, files)?;
        }
    }
    Ok(())
}

fn write_meta(w: &mut impl Write, meta: &MetaInfo) -> io::Result<()> {
    write!(w, "meta:\n")?;
    if meta.elapsed_ms != 0 {
        write!(w, "  elapsedMs: {}\n", meta.elapsed_ms)?;
    }
    if meta.timeout {
        write!(w, "  timeout: true\n")?;
    }
    if meta.files_scanned != 0 {
        write!(w, "  filesScanned: {}\n", meta.files_scanned)?;
    }
    if meta.files_matched != 0 {
        write!(w, "  filesMatched: {}\n", meta.files_matched)?;
    }
    if let Some(total) = meta.total_matches {
        write!(w, "  totalMatches: {}\n", total)?;
    }
    Ok(())
}

fn write_symbols(w: &mut impl Write, symbol_files: &[SymbolFile]) -> io::Result<()> {
    write!(w, "files:\n")?;
    for sf in symbol_files {
        write!(w, "- path: ")?;
        write_inline_string(w, &sf.path)?;
        write!(w, "\n")?;

        if let Some(ref error) = sf.error {
            write!(w, "  error: ")?;
            write_inline_string(w, error)?;
            write!(w, "\n")?;
        }

        if !sf.symbols.is_empty() {
            write!(w, "  symbols:\n")?;
            for sym in &sf.symbols {
                write_symbol_entry(w, sym)?;
            }
        }
    }
    Ok(())
}

fn write_symbol_entry(w: &mut impl Write, sym: &SymbolEntry) -> io::Result<()> {
    write!(w, "  - kind: {}\n", sym.kind)?;
    write!(w, "    name: ")?;
    write_inline_string(w, &sym.name)?;
    write!(w, "\n")?;
    write!(w, "    line: {}\n", sym.line)?;
    if let Some(ref vis) = sym.visibility {
        write!(w, "    visibility: {}\n", vis)?;
    }
    if let Some(ref parent) = sym.parent {
        write!(w, "    parent: ")?;
        write_inline_string(w, parent)?;
        write!(w, "\n")?;
    }
    write!(w, "    signature: ")?;
    write_inline_string(w, &sym.signature)?;
    write!(w, "\n")?;
    Ok(())
}

fn write_counts(w: &mut impl Write, counts: &[CountEntry]) -> io::Result<()> {
    write!(w, "files:\n")?;
    for entry in counts {
        write!(w, "- path: ")?;
        write_inline_string(w, &entry.path)?;
        write!(w, "\n")?;
        write!(w, "  count: {}\n", entry.count)?;
    }
    Ok(())
}

fn write_stats(w: &mut impl Write, stats: &StatsOutput) -> io::Result<()> {
    write!(w, "languages:\n")?;
    for lang in &stats.languages {
        write_lang_stats_entry(w, lang)?;
    }
    write!(w, "totals:\n")?;
    write!(w, "  files: {}\n", stats.totals.files)?;
    write!(w, "  lines: {}\n", stats.totals.lines)?;
    write!(w, "  bytes: {}\n", stats.totals.bytes)?;
    write!(w, "largest:\n")?;
    for file in &stats.largest {
        write_largest_entry(w, file)?;
    }
    Ok(())
}

fn write_lang_stats_entry(w: &mut impl Write, lang: &LangStats) -> io::Result<()> {
    write!(w, "- extension: ")?;
    write_inline_string(w, &lang.extension)?;
    write!(w, "\n")?;
    write!(w, "  files: {}\n", lang.files)?;
    write!(w, "  lines: {}\n", lang.lines)?;
    write!(w, "  bytes: {}\n", lang.bytes)?;
    Ok(())
}

fn write_largest_entry(w: &mut impl Write, file: &LargestFile) -> io::Result<()> {
    write!(w, "- path: ")?;
    write_inline_string(w, &file.path)?;
    write!(w, "\n")?;
    write!(w, "  lines: {}\n", file.lines)?;
    write!(w, "  bytes: {}\n", file.bytes)?;
    Ok(())
}

fn write_files(w: &mut impl Write, files: &[FileEntry]) -> io::Result<()> {
    write!(w, "files:\n")?;
    for file in files {
        write!(w, "- path: ")?;
        write_inline_string(w, &file.path)?;
        write!(w, "\n")?;

        if let Some(ref error) = file.error {
            write!(w, "  error: ")?;
            write_inline_string(w, error)?;
            write!(w, "\n")?;
        }

        if let Some(ref contents) = file.contents {
            write_block_scalar(w, "contents", contents, 2)?;
        }

        if let Some(ref chunks) = file.chunks {
            if !chunks.is_empty() {
                write!(w, "  chunks:\n")?;
                for chunk in chunks {
                    write_chunk(w, chunk)?;
                }
            }
        }
    }
    Ok(())
}

fn write_chunk(w: &mut impl Write, chunk: &FileChunk) -> io::Result<()> {
    write!(w, "  - startLine: {}\n", chunk.start_line)?;
    write!(w, "    endLine: {}\n", chunk.end_line)?;
    write_block_scalar(w, "content", &chunk.content, 4)?;
    Ok(())
}

fn write_tree_node(w: &mut impl Write, node: &ScanResult, indent: usize) -> io::Result<()> {
    write_indent(w, indent)?;
    write!(w, "name: ")?;
    write_inline_string(w, &node.name)?;
    write!(w, "\n")?;

    if let Some(ref files) = node.files {
        if !files.is_empty() {
            write_indent(w, indent)?;
            write!(w, "files:\n")?;
            for file in files {
                write_indent(w, indent)?;
                write!(w, "- ")?;
                write_inline_string(w, file)?;
                write!(w, "\n")?;
            }
        }
    }

    if let Some(ref children) = node.children {
        if !children.is_empty() {
            write_indent(w, indent)?;
            write!(w, "children:\n")?;
            for child in children {
                write_indent(w, indent)?;
                write!(w, "- name: ")?;
                write_inline_string(w, &child.name)?;
                write!(w, "\n")?;

                let child_indent = indent + 2;

                if let Some(ref files) = child.files {
                    if !files.is_empty() {
                        write_indent(w, child_indent)?;
                        write!(w, "files:\n")?;
                        for file in files {
                            write_indent(w, child_indent)?;
                            write!(w, "- ")?;
                            write_inline_string(w, file)?;
                            write!(w, "\n")?;
                        }
                    }
                }

                if let Some(ref grandchildren) = child.children {
                    if !grandchildren.is_empty() {
                        write_indent(w, child_indent)?;
                        write!(w, "children:\n")?;
                        for gc in grandchildren {
                            write_indent(w, child_indent)?;
                            write!(w, "- ")?;
                            write_tree_node_inline(w, gc, child_indent + 2)?;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn write_tree_node_inline(w: &mut impl Write, node: &ScanResult, indent: usize) -> io::Result<()> {
    write!(w, "name: ")?;
    write_inline_string(w, &node.name)?;
    write!(w, "\n")?;

    if let Some(ref files) = node.files {
        if !files.is_empty() {
            write_indent(w, indent)?;
            write!(w, "files:\n")?;
            for file in files {
                write_indent(w, indent)?;
                write!(w, "- ")?;
                write_inline_string(w, file)?;
                write!(w, "\n")?;
            }
        }
    }

    if let Some(ref children) = node.children {
        if !children.is_empty() {
            write_indent(w, indent)?;
            write!(w, "children:\n")?;
            for child in children {
                write_indent(w, indent)?;
                write!(w, "- ")?;
                write_tree_node_inline(w, child, indent + 2)?;
            }
        }
    }
    Ok(())
}

fn write_graph(w: &mut impl Write, graph: &[GraphEntry]) -> io::Result<()> {
    write!(w, "graph:\n")?;
    for entry in graph {
        write!(w, "- file: ")?;
        write_inline_string(w, &entry.file)?;
        write!(w, "\n")?;
        if entry.imports.is_empty() {
            write!(w, "  imports: []\n")?;
        } else {
            write!(w, "  imports:\n")?;
            for imp in &entry.imports {
                write!(w, "  - ")?;
                write_inline_string(w, imp)?;
                write!(w, "\n")?;
            }
        }
    }
    Ok(())
}

fn write_block_scalar(w: &mut impl Write, key: &str, content: &str, indent: usize) -> io::Result<()> {
    write_indent(w, indent)?;
    write!(w, "{}: |\n", key)?;
    for line in content.lines() {
        if line.is_empty() {
            write!(w, "\n")?;
        } else {
            write_indent(w, indent + 2)?;
            write!(w, "{}\n", line)?;
        }
    }
    Ok(())
}

fn write_scalar(w: &mut impl Write, key: &str, value: &str, indent: usize) -> io::Result<()> {
    write_indent(w, indent)?;
    write!(w, "{}: ", key)?;
    write_inline_string(w, value)?;
    write!(w, "\n")?;
    Ok(())
}

fn write_inline_string(w: &mut impl Write, value: &str) -> io::Result<()> {
    if value.is_empty() {
        return write!(w, "''");
    }

    if needs_quoting(value) {
        write!(w, "\"")?;
        for c in value.chars() {
            match c {
                '"' => write!(w, "\\\"")?,
                '\\' => write!(w, "\\\\")?,
                '\n' => write!(w, "\\n")?,
                '\r' => write!(w, "\\r")?,
                '\t' => write!(w, "\\t")?,
                _ => write!(w, "{}", c)?,
            }
        }
        write!(w, "\"")?;
    } else {
        write!(w, "{}", value)?;
    }
    Ok(())
}

fn needs_quoting(value: &str) -> bool {
    if value.is_empty() {
        return true;
    }

    let first = value.as_bytes()[0];
    if matches!(first, b'-' | b'[' | b']' | b'{' | b'}' | b'\'' | b'"' |
        b'!' | b'&' | b'*' | b'|' | b'>' | b'%' | b'@' | b'`' | b',' | b'?' | b'#') {
        return true;
    }

    match value {
        "true" | "false" | "null" | "True" | "False" | "Null" |
        "TRUE" | "FALSE" | "NULL" | "yes" | "no" | "Yes" | "No" |
        "YES" | "NO" | "on" | "off" | "On" | "Off" | "ON" | "OFF" => return true,
        _ => {}
    }

    for c in value.chars() {
        if matches!(c, ':' | '#' | '\n' | '\r') {
            return true;
        }
    }
    false
}

fn write_indent(w: &mut impl Write, n: usize) -> io::Result<()> {
        const SPACES: &[u8; 32] = b"                                ";
        if n <= SPACES.len() {
            w.write_all(&SPACES[..n])
        } else {
            for _ in 0..n {
                w.write_all(b" ")?;
            }
            Ok(())
        }
    }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;

    fn output_to_string(envelope: &OutputEnvelope) -> String {
        let mut buf = Vec::new();
        write_envelope(&mut buf, envelope).unwrap();
        String::from_utf8(buf).unwrap()
    }

    #[test]
    fn write_meta_basic() {
        let envelope = OutputEnvelope {
            meta: Some(MetaInfo {
                elapsed_ms: 42,
                timeout: false,
                files_scanned: 10,
                files_matched: 5,
                total_matches: None,
            }),
            files: None, tree: None, graph: None, symbols: None, counts: None, stats: None, error: None,
        };
        let s = output_to_string(&envelope);
        assert!(s.contains("meta:"));
        assert!(s.contains("elapsedMs: 42"));
        assert!(s.contains("filesScanned: 10"));
        assert!(s.contains("filesMatched: 5"));
    }

    #[test]
    fn write_meta_with_timeout() {
        let envelope = OutputEnvelope {
            meta: Some(MetaInfo {
                elapsed_ms: 100,
                timeout: true,
                files_scanned: 5,
                files_matched: 0,
                total_matches: None,
            }),
            files: None, tree: None, graph: None, symbols: None, counts: None, stats: None, error: None,
        };
        let s = output_to_string(&envelope);
        assert!(s.contains("timeout: true"));
    }

    #[test]
    fn write_error() {
        let envelope = OutputEnvelope {
            meta: None,
            files: None, tree: None, graph: None, symbols: None, counts: None, stats: None,
            error: Some("Something went wrong".to_owned()),
        };
        let s = output_to_string(&envelope);
        assert!(s.contains("error:"));
        assert!(s.contains("Something went wrong"));
    }

    #[test]
    fn write_files_with_contents() {
        let envelope = OutputEnvelope {
            meta: None,
            files: Some(vec![FileEntry {
                path: "src/main.rs".to_owned(),
                contents: Some("fn main() {}".to_owned()),
                error: None,
                chunks: None,
            }]),
            tree: None, graph: None, symbols: None, counts: None, stats: None, error: None,
        };
        let s = output_to_string(&envelope);
        assert!(s.contains("files:"));
        assert!(s.contains("path: src/main.rs"));
        assert!(s.contains("fn main() {}"));
    }

    #[test]
    fn write_files_with_chunks() {
        let envelope = OutputEnvelope {
            meta: None,
            files: Some(vec![FileEntry {
                path: "test.rs".to_owned(),
                contents: None,
                error: None,
                chunks: Some(vec![FileChunk {
                    start_line: 5,
                    end_line: 10,
                    content: "some code\n".to_owned(),
                }]),
            }]),
            tree: None, graph: None, symbols: None, counts: None, stats: None, error: None,
        };
        let s = output_to_string(&envelope);
        assert!(s.contains("startLine: 5"));
        assert!(s.contains("endLine: 10"));
        assert!(s.contains("some code"));
    }

    #[test]
    fn write_graph_output() {
        let envelope = OutputEnvelope {
            meta: None,
            files: None, tree: None,
            graph: Some(vec![
                GraphEntry { file: "a.rs".to_owned(), imports: vec!["b.rs".to_owned()] },
                GraphEntry { file: "c.rs".to_owned(), imports: vec![] },
            ]),
            symbols: None, counts: None, stats: None, error: None,
        };
        let s = output_to_string(&envelope);
        assert!(s.contains("graph:"));
        assert!(s.contains("file: a.rs"));
        assert!(s.contains("- b.rs"));
        assert!(s.contains("imports: []"));
    }

    #[test]
    fn write_symbols_output() {
        let envelope = OutputEnvelope {
            meta: None,
            files: None, tree: None, graph: None,
            symbols: Some(vec![SymbolFile {
                path: "test.rs".to_owned(),
                symbols: vec![SymbolEntry {
                    kind: "fn".to_owned(),
                    name: "main".to_owned(),
                    line: 1,
                    visibility: Some("pub".to_owned()),
                    parent: None,
                    signature: "pub fn main() {".to_owned(),
                }],
                error: None,
            }]),
            counts: None, stats: None, error: None,
        };
        let s = output_to_string(&envelope);
        assert!(s.contains("files:"));
        assert!(s.contains("path: test.rs"));
        assert!(s.contains("kind: fn"));
        assert!(s.contains("name: main"));
        assert!(s.contains("line: 1"));
        assert!(s.contains("visibility: pub"));
    }

    #[test]
    fn write_counts_output() {
        let envelope = OutputEnvelope {
            meta: None,
            files: None, tree: None, graph: None, symbols: None,
            counts: Some(vec![
                CountEntry { path: "a.rs".to_owned(), count: 5 },
                CountEntry { path: "b.rs".to_owned(), count: 3 },
            ]),
            stats: None, error: None,
        };
        let s = output_to_string(&envelope);
        assert!(s.contains("path: a.rs"));
        assert!(s.contains("count: 5"));
        assert!(s.contains("path: b.rs"));
        assert!(s.contains("count: 3"));
    }

    #[test]
    fn write_stats_output() {
        let envelope = OutputEnvelope {
            meta: None,
            files: None, tree: None, graph: None, symbols: None, counts: None,
            stats: Some(StatsOutput {
                languages: vec![LangStats {
                    extension: "rs".to_owned(),
                    files: 10,
                    lines: 1000,
                    bytes: 50000,
                }],
                totals: StatsTotals { files: 10, lines: 1000, bytes: 50000 },
                largest: vec![LargestFile { path: "big.rs".to_owned(), lines: 500, bytes: 25000 }],
            }),
            error: None,
        };
        let s = output_to_string(&envelope);
        assert!(s.contains("languages:"));
        assert!(s.contains("extension: rs"));
        assert!(s.contains("files: 10"));
        assert!(s.contains("totals:"));
        assert!(s.contains("largest:"));
        assert!(s.contains("path: big.rs"));
    }

    #[test]
    fn write_tree_output() {
        let envelope = OutputEnvelope {
            meta: None,
            files: None,
            tree: Some(ScanResult {
                name: "project".to_owned(),
                files: Some(vec!["README.md".to_owned()]),
                children: Some(vec![ScanResult {
                    name: "src".to_owned(),
                    files: Some(vec!["main.rs".to_owned()]),
                    children: None,
                }]),
            }),
            graph: None, symbols: None, counts: None, stats: None, error: None,
        };
        let s = output_to_string(&envelope);
        assert!(s.contains("tree:"));
        assert!(s.contains("name: project"));
        assert!(s.contains("README.md"));
        assert!(s.contains("name: src"));
        assert!(s.contains("main.rs"));
    }

    #[test]
    fn needs_quoting_special_first_chars() {
        assert!(needs_quoting("-value"));
        assert!(needs_quoting("[list]"));
        assert!(needs_quoting("{map}"));
        assert!(needs_quoting("'quoted'"));
        assert!(needs_quoting("\"quoted\""));
        assert!(needs_quoting("!tag"));
        assert!(needs_quoting("&anchor"));
        assert!(needs_quoting("*alias"));
        assert!(needs_quoting("|block"));
        assert!(needs_quoting(">folded"));
        assert!(needs_quoting("%directive"));
        assert!(needs_quoting("@value"));
        assert!(needs_quoting("`tick"));
        assert!(needs_quoting(",value"));
        assert!(needs_quoting("?key"));
        assert!(needs_quoting("#comment"));
    }

    #[test]
    fn needs_quoting_yaml_booleans() {
        assert!(needs_quoting("true"));
        assert!(needs_quoting("false"));
        assert!(needs_quoting("null"));
        assert!(needs_quoting("True"));
        assert!(needs_quoting("False"));
        assert!(needs_quoting("yes"));
        assert!(needs_quoting("no"));
        assert!(needs_quoting("on"));
        assert!(needs_quoting("off"));
    }

    #[test]
    fn needs_quoting_colon_and_hash() {
        assert!(needs_quoting("key: value"));
        assert!(needs_quoting("value # comment"));
    }

    #[test]
    fn needs_quoting_newlines() {
        assert!(needs_quoting("line1\nline2"));
        assert!(needs_quoting("line1\rline2"));
    }

    #[test]
    fn no_quoting_for_simple_strings() {
        assert!(!needs_quoting("hello"));
        assert!(!needs_quoting("main.rs"));
        assert!(!needs_quoting("src/lang/rust.rs"));
        assert!(!needs_quoting("pub fn main()"));
    }

    #[test]
    fn needs_quoting_empty() {
        assert!(needs_quoting(""));
    }

    #[test]
    fn inline_string_quotes_values_with_colons() {
        let mut buf = Vec::new();
        write_inline_string(&mut buf, "key: value").unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.starts_with('"'));
        assert!(s.ends_with('"'));
    }

    #[test]
    fn inline_string_escapes_backslash_and_quotes() {
        let mut buf = Vec::new();
        // starts with " which triggers quoting, then contains backslash
        write_inline_string(&mut buf, "\"hello\\world\"").unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("\\\\"));
    }

    #[test]
    fn inline_string_escapes_newlines() {
        let mut buf = Vec::new();
        write_inline_string(&mut buf, "line1\nline2").unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("\\n"));
    }

    #[test]
    fn inline_string_empty_value() {
        let mut buf = Vec::new();
        write_inline_string(&mut buf, "").unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert_eq!(s, "''");
    }

    #[test]
    fn inline_string_plain_value() {
        let mut buf = Vec::new();
        write_inline_string(&mut buf, "simple").unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert_eq!(s, "simple");
    }

    #[test]
    fn write_indent_small() {
        let mut buf = Vec::new();
        write_indent(&mut buf, 4).unwrap();
        assert_eq!(buf, b"    ");
    }

    #[test]
    fn write_indent_large() {
        let mut buf = Vec::new();
        write_indent(&mut buf, 40).unwrap();
        assert_eq!(buf.len(), 40);
        assert!(buf.iter().all(|&b| b == b' '));
    }
}
