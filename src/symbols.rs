use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use rayon::prelude::*;

use crate::file_reader;
use crate::lang;
use crate::models::SymbolFile;
use crate::path_helper;

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

    let content = match file_reader::read_file(path) {
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

    let symbols = handler.extract_symbols(&content);

    if symbols.is_empty() {
        return None;
    }

    Some(SymbolFile {
        path: relative,
        symbols,
        error: None,
    })
}
