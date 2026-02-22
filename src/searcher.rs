use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use rayon::prelude::*;
use regex::Regex;

use crate::file_reader;
use crate::models::{FileChunk, FileEntry};
use crate::path_helper;

pub enum Matcher {
    Literal(String),
    MultiTerm(Vec<String>),
    Regex(Regex),
}

impl Matcher {
    pub fn build(pattern: &str, is_regex: bool) -> Result<Self, String> {
        if is_regex {
            Regex::new(pattern)
                .map(Matcher::Regex)
                .map_err(|e| format!("Invalid regex: {}", e))
        } else if pattern.contains('|') {
            let terms: Vec<String> = pattern
                .split('|')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_owned())
                .collect();
            if terms.is_empty() {
                Err("Empty search pattern".into())
            } else {
                Ok(Matcher::MultiTerm(terms))
            }
        } else {
            Ok(Matcher::Literal(pattern.to_owned()))
        }
    }

    #[inline]
    pub fn is_match(&self, line: &str) -> bool {
        match self {
            Matcher::Literal(pat) => contains_ci(line, pat),
            Matcher::MultiTerm(terms) => terms.iter().any(|t| contains_ci(line, t)),
            Matcher::Regex(re) => re.is_match(line),
        }
    }
}

#[inline]
fn contains_ci(haystack: &str, needle: &str) -> bool {
    if needle.len() > haystack.len() {
        return false;
    }
    let h = haystack.as_bytes();
    let n = needle.as_bytes();
    let end = h.len() - n.len() + 1;
    'outer: for i in 0..end {
        for j in 0..n.len() {
            if h[i + j].to_ascii_lowercase() != n[j].to_ascii_lowercase() {
                continue 'outer;
            }
        }
        return true;
    }
    false
}

pub fn search_files(
    file_paths: &[String],
    root: &Path,
    matcher: &Matcher,
    line_numbers: bool,
    cancelled: &AtomicBool,
) -> Vec<FileEntry> {
    let mut results: Vec<FileEntry> = file_paths
        .par_iter()
        .filter_map(|file_path| {
            if cancelled.load(Ordering::Relaxed) {
                return None;
            }
            process_file(file_path, root, matcher, line_numbers)
        })
        .collect();

    results.sort_by(|a, b| a.path.to_ascii_lowercase().cmp(&b.path.to_ascii_lowercase()));
    results
}

fn process_file(
    file_path: &str,
    root: &Path,
    matcher: &Matcher,
    line_numbers: bool,
) -> Option<FileEntry> {
    let path = Path::new(file_path);
    let relative = path_helper::normalized_relative(root, path);

    if fs::metadata(path).is_err() {
        return Some(FileEntry {
            path: relative,
            contents: None,
            error: Some("File not found".to_string()),
            chunks: None,
        });
    }

    let content = match file_reader::read_file(path) {
        Ok(Some(c)) => c,
        Ok(None) => return None,
        Err(e) => return Some(FileEntry {
            path: relative,
            contents: None,
            error: Some(e),
            chunks: None,
        }),
    };

    search_content(&content, &relative, matcher, line_numbers)
}

fn search_content(
    content: &str,
    relative: &str,
    matcher: &Matcher,
    line_numbers: bool,
) -> Option<FileEntry> {
    let lines: Vec<&str> = content.lines().collect();

    let has_match = lines.iter().any(|line| matcher.is_match(line));
    if !has_match {
        return None;
    }

    let mut output = String::new();
    for (i, line) in lines.iter().enumerate() {
        if line_numbers {
            let line_num = i + 1;
            output.push_str(&line_num.to_string());
            output.push_str(".  ");
        }
        output.push_str(line);
        output.push('\n');
    }

    Some(FileEntry {
        path: relative.to_owned(),
        contents: Some(output),
        error: None,
        chunks: None,
    })
}

#[allow(dead_code)]
fn merge_ranges(
    matching_indices: &[usize],
    pad: usize,
    line_count: usize,
) -> Vec<(usize, usize)> {
    let mut ranges: Vec<(usize, usize)> = Vec::with_capacity(matching_indices.len());

    for &idx in matching_indices {
        let start = idx.saturating_sub(pad);
        let end = (idx + pad).min(line_count.saturating_sub(1));

        if let Some(last) = ranges.last_mut() {
            if start <= last.1 + 1 {
                last.1 = last.1.max(end);
                continue;
            }
        }
        ranges.push((start, end));
    }
    ranges
}

pub fn build_chunks(lines: &[&str], ranges: &[(usize, usize)], line_numbers: bool) -> Vec<FileChunk> {
    let mut chunks = Vec::with_capacity(ranges.len());

    for &(start, end) in ranges {
        let mut content = String::new();
        for i in start..=end {
            if i < lines.len() {
                if line_numbers {
                    let line_num = i + 1;
                    content.push_str(&line_num.to_string());
                    content.push_str(".  ");
                }
                content.push_str(lines[i]);
                content.push('\n');
            }
        }

        chunks.push(FileChunk {
            start_line: start + 1,
            end_line: (end + 1).min(lines.len()),
            content,
        });
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_matcher_exact() {
        let m = Matcher::build("hello", false).unwrap();
        assert!(m.is_match("hello world"));
        assert!(!m.is_match("goodbye world"));
    }

    #[test]
    fn literal_matcher_case_insensitive() {
        let m = Matcher::build("Hello", false).unwrap();
        assert!(m.is_match("HELLO WORLD"));
        assert!(m.is_match("hello world"));
        assert!(m.is_match("hElLo world"));
    }

    #[test]
    fn multi_term_matcher() {
        let m = Matcher::build("foo|bar", false).unwrap();
        assert!(m.is_match("this has foo"));
        assert!(m.is_match("this has bar"));
        assert!(!m.is_match("this has baz"));
    }

    #[test]
    fn multi_term_case_insensitive() {
        let m = Matcher::build("FOO|BAR", false).unwrap();
        assert!(m.is_match("foo here"));
        assert!(m.is_match("bar here"));
    }

    #[test]
    fn regex_matcher() {
        let m = Matcher::build(r"fn \w+\(", true).unwrap();
        assert!(m.is_match("fn hello("));
        assert!(!m.is_match("let x = 5"));
    }

    #[test]
    fn regex_invalid_returns_error() {
        let result = Matcher::build("[invalid", true);
        assert!(result.is_err());
    }

    #[test]
    fn empty_multi_term_error() {
        let result = Matcher::build("|", false);
        assert!(result.is_err());
    }

    #[test]
    fn contains_ci_basic() {
        assert!(contains_ci("Hello World", "hello"));
        assert!(contains_ci("Hello World", "WORLD"));
        assert!(!contains_ci("Hello", "xyz"));
    }

    #[test]
    fn contains_ci_needle_longer_than_haystack() {
        assert!(!contains_ci("ab", "abcdef"));
    }

    #[test]
    fn contains_ci_empty() {
        assert!(contains_ci("anything", ""));
    }

    #[test]
    fn merge_ranges_no_overlap() {
        let indices = vec![0, 10, 20];
        let result = merge_ranges(&indices, 2, 30);
        assert_eq!(result, vec![(0, 2), (8, 12), (18, 22)]);
    }

    #[test]
    fn merge_ranges_overlap() {
        let indices = vec![5, 7];
        let result = merge_ranges(&indices, 3, 30);
        assert_eq!(result, vec![(2, 10)]);
    }

    #[test]
    fn merge_ranges_clamp_start() {
        let indices = vec![0];
        let result = merge_ranges(&indices, 5, 10);
        assert_eq!(result, vec![(0, 5)]);
    }

    #[test]
    fn merge_ranges_clamp_end() {
        let indices = vec![9];
        let result = merge_ranges(&indices, 5, 10);
        assert_eq!(result, vec![(4, 9)]);
    }

    #[test]
    fn merge_ranges_no_pad() {
        let indices = vec![3, 7];
        let result = merge_ranges(&indices, 0, 10);
        assert_eq!(result, vec![(3, 3), (7, 7)]);
    }

    #[test]
    fn build_chunks_with_line_numbers() {
        let lines = vec!["line0", "line1", "line2", "line3", "line4"];
        let ranges = vec![(1, 3)];
        let chunks = build_chunks(&lines, &ranges, true);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].start_line, 2);
        assert_eq!(chunks[0].end_line, 4);
        assert!(chunks[0].content.contains("2.  line1"));
        assert!(chunks[0].content.contains("3.  line2"));
        assert!(chunks[0].content.contains("4.  line3"));
    }

    #[test]
    fn build_chunks_without_line_numbers() {
        let lines = vec!["line0", "line1", "line2"];
        let ranges = vec![(0, 2)];
        let chunks = build_chunks(&lines, &ranges, false);
        assert_eq!(chunks.len(), 1);
        assert!(!chunks[0].content.contains("1."));
        assert!(chunks[0].content.contains("line0"));
        assert!(chunks[0].content.contains("line2"));
    }

    #[test]
    fn build_chunks_multiple_ranges() {
        let lines = vec!["a", "b", "c", "d", "e", "f"];
        let ranges = vec![(0, 1), (4, 5)];
        let chunks = build_chunks(&lines, &ranges, false);
        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].content.contains("a"));
        assert!(chunks[1].content.contains("e"));
    }

    #[test]
    fn multi_term_trims_whitespace() {
        let m = Matcher::build("foo | bar | baz", false).unwrap();
        assert!(m.is_match("contains foo here"));
        assert!(m.is_match("contains bar here"));
        assert!(m.is_match("contains baz here"));
    }
}
