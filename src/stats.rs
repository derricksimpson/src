use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use memmap2::Mmap;
use rayon::prelude::*;

use crate::models::{LangStats, LargestFile, StatsOutput, StatsTotals};
use crate::path_helper;

const MMAP_THRESHOLD: u64 = 64 * 1024;
const BINARY_CHECK_SIZE: usize = 8192;

struct FileInfo {
    path: String,
    extension: String,
    lines: usize,
    bytes: u64,
}

pub fn compute_stats(
    file_paths: &[String],
    root: &Path,
    cancelled: &AtomicBool,
) -> StatsOutput {
    let infos: Vec<FileInfo> = file_paths
        .par_iter()
        .filter_map(|file_path| {
            if cancelled.load(Ordering::Relaxed) {
                return None;
            }
            process_file(file_path, root)
        })
        .collect();

    let mut by_ext: HashMap<String, (usize, usize, u64)> = HashMap::new();
    let mut total_files = 0usize;
    let mut total_lines = 0usize;
    let mut total_bytes = 0u64;

    let mut all_files: Vec<(&str, usize, u64)> = Vec::with_capacity(infos.len());

    for info in &infos {
        let entry = by_ext.entry(info.extension.clone()).or_insert((0, 0, 0));
        entry.0 += 1;
        entry.1 += info.lines;
        entry.2 += info.bytes;

        total_files += 1;
        total_lines += info.lines;
        total_bytes += info.bytes;

        all_files.push((&info.path, info.lines, info.bytes));
    }

    let mut languages: Vec<LangStats> = by_ext
        .into_iter()
        .map(|(ext, (files, lines, bytes))| LangStats {
            extension: ext,
            files,
            lines,
            bytes,
        })
        .collect();
    languages.sort_by(|a, b| b.lines.cmp(&a.lines));

    all_files.sort_by(|a, b| b.2.cmp(&a.2));
    let largest: Vec<LargestFile> = all_files
        .iter()
        .take(10)
        .map(|(path, lines, bytes)| LargestFile {
            path: path.to_string(),
            lines: *lines,
            bytes: *bytes,
        })
        .collect();

    StatsOutput {
        languages,
        totals: StatsTotals {
            files: total_files,
            lines: total_lines,
            bytes: total_bytes,
        },
        largest,
    }
}

fn process_file(file_path: &str, root: &Path) -> Option<FileInfo> {
    let path = Path::new(file_path);
    let relative = path_helper::normalized_relative(root, path);
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let metadata = std::fs::metadata(path).ok()?;
    let byte_size = metadata.len();

    let line_count = count_lines(path, byte_size);

    Some(FileInfo {
        path: relative,
        extension,
        lines: line_count,
        bytes: byte_size,
    })
}

fn count_lines(path: &Path, size: u64) -> usize {
    if size == 0 {
        return 0;
    }

    if size >= MMAP_THRESHOLD {
        if let Ok(file) = File::open(path) {
            if let Ok(mmap) = unsafe { Mmap::map(&file) } {
                let data = &mmap[..];
                if is_binary(data) {
                    return 0;
                }
                return bytecount_newlines(data);
            }
        }
    }

    if let Ok(file) = File::open(path) {
        let mut reader = BufReader::with_capacity(64 * 1024, file);
        let mut check_buf = [0u8; BINARY_CHECK_SIZE];
        if let Ok(n) = reader.read(&mut check_buf) {
            if is_binary(&check_buf[..n]) {
                return 0;
            }
            let mut count = memchr_count(b'\n', &check_buf[..n]);
            let mut buf = [0u8; 32 * 1024];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => count += memchr_count(b'\n', &buf[..n]),
                    Err(_) => break,
                }
            }
            if n > 0 && check_buf[n - 1] != b'\n' {
                count += 1;
            }
            return count;
        }
    }

    0
}

fn memchr_count(needle: u8, haystack: &[u8]) -> usize {
    haystack.iter().filter(|&&b| b == needle).count()
}

fn bytecount_newlines(data: &[u8]) -> usize {
    let mut count = memchr_count(b'\n', data);
    if !data.is_empty() && *data.last().unwrap() != b'\n' {
        count += 1;
    }
    count
}

fn is_binary(data: &[u8]) -> bool {
    let check_len = data.len().min(BINARY_CHECK_SIZE);
    data[..check_len].contains(&0)
}
