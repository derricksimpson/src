use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use rayon::prelude::*;

use crate::file_reader;
use crate::lang;
use crate::lang::common;
use crate::models::SymbolFile;
use crate::path_helper;
use crate::searcher::Matcher;

pub fn extract_symbols(
    file_paths: &[String],
    root: &Path,
    cancelled: &AtomicBool,
    with_comments: bool,
) -> Vec<SymbolFile> {
    let mut results: Vec<SymbolFile> = file_paths
        .par_iter()
        .filter_map(|file_path| {
            if cancelled.load(Ordering::Relaxed) {
                return None;
            }
            process_file(file_path, root, with_comments)
        })
        .collect();

    results.sort_by(|a, b| a.path.to_ascii_lowercase().cmp(&b.path.to_ascii_lowercase()));
    results
}

fn process_file(file_path: &str, root: &Path, with_comments: bool) -> Option<SymbolFile> {
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

    let mut symbols = handler.extract_symbols(&content);

    if with_comments && !symbols.is_empty() {
        let lines: Vec<&str> = content.lines().collect();
        let is_python = matches!(ext.to_ascii_lowercase().as_str(), "py");
        for sym in &mut symbols {
            if sym.line > 0 && sym.line <= lines.len() {
                let comment = common::extract_preceding_comment(&lines, sym.line - 1);
                if comment.is_some() {
                    sym.comment = comment;
                } else if is_python {
                    sym.comment = common::extract_docstring_after(&lines, sym.line - 1);
                }
            }
        }
    }

    if symbols.is_empty() {
        return None;
    }

    Some(SymbolFile {
        path: relative,
        symbols,
        error: None,
    })
}

pub fn filter_symbols(
    symbol_files: Vec<SymbolFile>,
    matcher: &Matcher,
) -> (Vec<SymbolFile>, usize) {
    let mut total_matches = 0usize;
    let filtered: Vec<SymbolFile> = symbol_files
        .into_iter()
        .filter_map(|mut sf| {
            sf.symbols.retain(|sym| matcher.is_match(&sym.name));
            if sf.symbols.is_empty() && sf.error.is_none() {
                return None;
            }
            total_matches += sf.symbols.len();
            Some(sf)
        })
        .collect();
    (filtered, total_matches)
}
