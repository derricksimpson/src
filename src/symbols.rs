use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use memmap2::Mmap;
use rayon::prelude::*;

use crate::lang;
use crate::models::{SymbolEntry, SymbolFile};
use crate::path_helper;

const MMAP_THRESHOLD: u64 = 64 * 1024;
const BINARY_CHECK_SIZE: usize = 8192;

pub fn extract_symbols(
    file_paths: &[String],
    root: &Path,
    cancelled: &AtomicBool,
) -> Vec<SymbolFile> {
    let mut results: Vec<SymbolFile> = file_paths
        .par_iter()
        .filter_map(|file_path| {
            if cancelled.load(Ordering::Relaxed) {
                return None;
            }
            process_file(file_path, root)
        })
        .collect();

    results.sort_by(|a, b| a.path.to_ascii_lowercase().cmp(&b.path.to_ascii_lowercase()));
    results
}

fn process_file(file_path: &str, root: &Path) -> Option<SymbolFile> {
    let path = Path::new(file_path);
    let relative = path_helper::normalized_relative(root, path);

    let ext = path.extension()?.to_str()?;
    let handler = lang::get_symbol_handler(ext)?;

    let content = match read_file(path) {
        Ok(Some(c)) => c,
        Ok(None) => return None,
        Err(e) => {
            return Some(SymbolFile {
                path: relative,
                symbols: Vec::new(),
                error: Some(e),
            });
        }
    };

    let raw_symbols = handler.extract_symbols(&content);
    let symbols: Vec<SymbolEntry> = raw_symbols
        .into_iter()
        .map(|s| SymbolEntry {
            kind: s.kind.to_owned(),
            name: s.name,
            line: s.line,
            visibility: s.visibility.map(|v| v.to_owned()),
            parent: s.parent,
            signature: s.signature,
        })
        .collect();

    if symbols.is_empty() {
        return None;
    }

    Some(SymbolFile {
        path: relative,
        symbols,
        error: None,
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
