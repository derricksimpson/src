use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use memmap2::Mmap;
use rayon::prelude::*;

use crate::models::FileEntry;
use crate::path_helper;
use crate::searcher;

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

const MMAP_THRESHOLD: u64 = 64 * 1024;
const BINARY_CHECK_SIZE: usize = 8192;

fn extract_file(
    root: &Path,
    rel_path: &str,
    ranges: &[(usize, usize)],
    line_numbers: bool,
) -> FileEntry {
    let full_path = root.join(rel_path);

    let metadata = match std::fs::metadata(&full_path) {
        Ok(m) => m,
        Err(_) => return FileEntry {
            path: rel_path.to_owned(),
            contents: None,
            error: Some(format!("File not found: {}", rel_path)),
            chunks: None,
        },
    };

    let content = if metadata.len() >= MMAP_THRESHOLD {
        read_file_mmap(&full_path)
    } else {
        read_file_buffered(&full_path)
    };

    let content = match content {
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

fn read_file_mmap(path: &Path) -> Result<Option<String>, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mmap = unsafe { Mmap::map(&file) }.map_err(|e| e.to_string())?;
    let data = &mmap[..];
    if is_binary(data) {
        return Ok(None);
    }
    let s = std::str::from_utf8(data).map_err(|_| "Not valid UTF-8".to_string())?;
    Ok(Some(s.to_owned()))
}

fn read_file_buffered(path: &Path) -> Result<Option<String>, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mut reader = BufReader::with_capacity(64 * 1024, file);
    let mut check_buf = [0u8; BINARY_CHECK_SIZE];
    let n = reader.read(&mut check_buf).map_err(|e| e.to_string())?;
    if is_binary(&check_buf[..n]) {
        return Ok(None);
    }
    let mut all = Vec::from(&check_buf[..n]);
    reader.read_to_end(&mut all).map_err(|e| e.to_string())?;
    let s = std::str::from_utf8(&all).map_err(|_| "Not valid UTF-8".to_string())?;
    Ok(Some(s.to_owned()))
}

fn is_binary(data: &[u8]) -> bool {
    let check_len = data.len().min(BINARY_CHECK_SIZE);
    data[..check_len].contains(&0)
}
