use std::io::{self, Write, BufWriter};

use crate::models::{FileChunk, FileEntry, GraphEntry, MetaInfo, OutputEnvelope, ScanResult};

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
