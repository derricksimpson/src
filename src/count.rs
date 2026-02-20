use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use memmap2::Mmap;
use rayon::prelude::*;

use crate::models::CountEntry;
use crate::path_helper;
use crate::searcher::Matcher;

const MMAP_THRESHOLD: u64 = 64 * 1024;
const BINARY_CHECK_SIZE: usize = 8192;

pub fn count_matches(
    file_paths: &[String],
    root: &Path,
    matcher: &Matcher,
    cancelled: &AtomicBool,
) -> (Vec<CountEntry>, usize) {
    let entries: Vec<CountEntry> = file_paths
        .par_iter()
        .filter_map(|file_path| {
            if cancelled.load(Ordering::Relaxed) {
                return None;
            }
            process_file(file_path, root, matcher)
        })
        .collect();

    let mut entries = entries;
    let total: usize = entries.iter().map(|e| e.count).sum();
    entries.sort_by(|a, b| a.path.to_ascii_lowercase().cmp(&b.path.to_ascii_lowercase()));
    (entries, total)
}

fn process_file(file_path: &str, root: &Path, matcher: &Matcher) -> Option<CountEntry> {
    let path = Path::new(file_path);
    let relative = path_helper::normalized_relative(root, path);

    let content = match read_file(path) {
        Ok(Some(c)) => c,
        _ => return None,
    };

    let count = content.lines().filter(|line| matcher.is_match(line)).count();
    if count == 0 {
        return None;
    }

    Some(CountEntry {
        path: relative,
        count,
    })
}

fn read_file(path: &Path) -> Result<Option<String>, String> {
    let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if metadata.len() == 0 {
        return Ok(None);
    }
    if metadata.len() >= MMAP_THRESHOLD {
        read_file_mmap(path)
    } else {
        read_file_buffered(path)
    }
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
