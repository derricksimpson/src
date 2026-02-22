use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use rayon::prelude::*;

use crate::file_reader;
use crate::models::FileEntry;
use crate::path_helper;
use crate::searcher;

#[derive(Debug)]
pub struct LineSpec {
    pub path: String,
    pub start: usize,
    pub end: usize,
}

pub fn parse_line_specs(raw: &[String], root: &Path) -> Result<Vec<LineSpec>, String> {
    let mut specs = Vec::with_capacity(raw.len());
    for s in raw {
        let parts: Vec<&str> = s.rsplitn(3, ':').collect();
        if parts.len() != 3 {
            return Err(format!("Invalid line spec: '{}'. Expected format: path:start:end", s));
        }
        let end_str = parts[0];
        let start_str = parts[1];
        let file_path = parts[2];

        let start: usize = start_str.parse()
            .map_err(|_| format!("Invalid line spec: '{}' — start line '{}' is not an integer", s, start_str))?;
        let end: usize = end_str.parse()
            .map_err(|_| format!("Invalid line spec: '{}' — end line '{}' is not an integer", s, end_str))?;

        if start == 0 || end == 0 {
            return Err(format!("Invalid line spec: '{}' — line numbers are 1-based", s));
        }

        let (start, end) = if start > end { (end, start) } else { (start, end) };

        let resolved = root.join(file_path);
        let norm = path_helper::normalized_relative(root, &resolved);

        specs.push(LineSpec { path: norm, start, end });
    }
    Ok(specs)
}

pub fn extract_lines(
    specs: &[LineSpec],
    root: &Path,
    line_numbers: bool,
    cancelled: &AtomicBool,
) -> Vec<FileEntry> {
    let mut grouped: HashMap<&str, Vec<(usize, usize)>> = HashMap::new();
    for spec in specs {
        grouped.entry(&spec.path).or_default().push((spec.start, spec.end));
    }

    let mut groups: Vec<(&str, Vec<(usize, usize)>)> = grouped.into_iter().collect();
    groups.sort_by(|a, b| a.0.to_ascii_lowercase().cmp(&b.0.to_ascii_lowercase()));

    let results: Vec<FileEntry> = groups
        .par_iter()
        .filter_map(|(rel_path, ranges)| {
            if cancelled.load(Ordering::Relaxed) {
                return None;
            }
            Some(extract_file(root, rel_path, ranges, line_numbers))
        })
        .collect();

    let mut results = results;
    results.sort_by(|a, b| a.path.to_ascii_lowercase().cmp(&b.path.to_ascii_lowercase()));
    results
}

fn extract_file(
    root: &Path,
    rel_path: &str,
    ranges: &[(usize, usize)],
    line_numbers: bool,
) -> FileEntry {
    let full_path = root.join(rel_path);

    let content = match file_reader::read_file(&full_path) {
        Ok(Some(c)) => c,
        Ok(None) => return FileEntry {
            path: rel_path.to_owned(),
            contents: None,
            error: None,
            chunks: None,
        },
        Err(e) => return FileEntry {
            path: rel_path.to_owned(),
            contents: None,
            error: Some(e),
            chunks: None,
        },
    };

    let lines: Vec<&str> = content.lines().collect();
    let line_count = lines.len();

    let mut merged = merge_and_sort_ranges(ranges, line_count);

    let zero_ranges: Vec<(usize, usize)> = merged
        .drain(..)
        .map(|(s, e)| (s - 1, e - 1))
        .collect();

    let chunks = searcher::build_chunks(&lines, &zero_ranges, line_numbers);

    if chunks.len() == 1 && chunks[0].start_line == 1 && chunks[0].end_line == line_count {
        FileEntry {
            path: rel_path.to_owned(),
            contents: Some(chunks.into_iter().next().unwrap().content),
            error: None,
            chunks: None,
        }
    } else {
        FileEntry {
            path: rel_path.to_owned(),
            contents: None,
            error: None,
            chunks: Some(chunks),
        }
    }
}

fn merge_and_sort_ranges(ranges: &[(usize, usize)], line_count: usize) -> Vec<(usize, usize)> {
    let mut clamped: Vec<(usize, usize)> = ranges
        .iter()
        .map(|&(s, e)| {
            let s = s.max(1).min(line_count);
            let e = e.max(1).min(line_count);
            if s > e { (e, s) } else { (s, e) }
        })
        .collect();

    clamped.sort_by_key(|&(s, _)| s);

    let mut merged: Vec<(usize, usize)> = Vec::with_capacity(clamped.len());
    for (s, e) in clamped {
        if let Some(last) = merged.last_mut() {
            if s <= last.1 + 1 {
                last.1 = last.1.max(e);
                continue;
            }
        }
        merged.push((s, e));
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_line_spec() {
        let root = Path::new("/project");
        let specs = parse_line_specs(&["src/main.rs:1:20".to_owned()], root).unwrap();
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].start, 1);
        assert_eq!(specs[0].end, 20);
    }

    #[test]
    fn parse_line_spec_auto_swaps_reversed() {
        let root = Path::new("/project");
        let specs = parse_line_specs(&["src/main.rs:20:1".to_owned()], root).unwrap();
        assert_eq!(specs[0].start, 1);
        assert_eq!(specs[0].end, 20);
    }

    #[test]
    fn parse_line_spec_invalid_format() {
        let root = Path::new("/project");
        let result = parse_line_specs(&["badformat".to_owned()], root);
        assert!(result.is_err());
    }

    #[test]
    fn parse_line_spec_zero_line_number() {
        let root = Path::new("/project");
        let result = parse_line_specs(&["file.rs:0:10".to_owned()], root);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("1-based"));
    }

    #[test]
    fn parse_line_spec_non_integer() {
        let root = Path::new("/project");
        let result = parse_line_specs(&["file.rs:abc:10".to_owned()], root);
        assert!(result.is_err());
    }

    #[test]
    fn parse_multiple_specs() {
        let root = Path::new("/project");
        let specs = parse_line_specs(
            &["a.rs:1:5".to_owned(), "b.rs:10:20".to_owned()],
            root,
        ).unwrap();
        assert_eq!(specs.len(), 2);
    }

    #[test]
    fn merge_and_sort_overlapping_ranges() {
        let ranges = vec![(1, 5), (3, 8), (10, 15)];
        let merged = merge_and_sort_ranges(&ranges, 20);
        assert_eq!(merged, vec![(1, 8), (10, 15)]);
    }

    #[test]
    fn merge_and_sort_adjacent_ranges() {
        let ranges = vec![(1, 5), (6, 10)];
        let merged = merge_and_sort_ranges(&ranges, 20);
        assert_eq!(merged, vec![(1, 10)]);
    }

    #[test]
    fn merge_and_sort_clamps_to_line_count() {
        let ranges = vec![(1, 100)];
        let merged = merge_and_sort_ranges(&ranges, 10);
        assert_eq!(merged, vec![(1, 10)]);
    }

    #[test]
    fn merge_and_sort_single_range() {
        let ranges = vec![(5, 10)];
        let merged = merge_and_sort_ranges(&ranges, 20);
        assert_eq!(merged, vec![(5, 10)]);
    }

    #[test]
    fn merge_and_sort_reversed_range() {
        let ranges = vec![(10, 5)];
        let merged = merge_and_sort_ranges(&ranges, 20);
        assert_eq!(merged, vec![(5, 10)]);
    }

    #[test]
    fn parse_line_spec_with_colon_in_path() {
        let root = Path::new("/project");
        let specs = parse_line_specs(&["C:\\src\\main.rs:1:20".to_owned()], root);
        assert!(specs.is_ok());
    }
}
