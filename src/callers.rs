use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use rayon::prelude::*;

use crate::file_reader;
use crate::models::{CallerDeclaration, CallerEntry, CallerFile, CallersOutput};
use crate::path_helper;
use crate::searcher::Matcher;
use crate::symbols;

pub fn find_callers(
    file_paths: &[String],
    root: &Path,
    name: &str,
    is_regex: bool,
    include_tests: bool,
    cancelled: &AtomicBool,
) -> Result<CallersOutput, String> {
    let symbol_files = symbols::extract_symbols(file_paths, root, cancelled, false, include_tests);

    let mut declarations: Vec<CallerDeclaration> = Vec::new();
    for sf in &symbol_files {
        for sym in &sf.symbols {
            if sym.name == name {
                declarations.push(CallerDeclaration {
                    path: sf.path.clone(),
                    line: sym.line,
                    signature: sym.signature.clone(),
                });
            }
        }
    }

    let matcher = Matcher::build(name, is_regex)?;

    let decl_set: Vec<(String, usize)> = declarations
        .iter()
        .map(|d| (d.path.clone(), d.line))
        .collect();

    let results: Vec<CallerFile> = file_paths
        .par_iter()
        .filter_map(|file_path| {
            if cancelled.load(Ordering::Relaxed) {
                return None;
            }
            let path = Path::new(file_path);
            let relative = path_helper::normalized_relative(root, path);

            let content = match file_reader::read_file(path) {
                Ok(Some(c)) => c,
                _ => return None,
            };

            let mut sites: Vec<CallerEntry> = Vec::new();
            for (i, line) in content.lines().enumerate() {
                let line_num = i + 1;
                if matcher.is_match(line) {
                    let is_decl = decl_set.iter().any(|(dp, dl)| dp == &relative && *dl == line_num);
                    if !is_decl {
                        sites.push(CallerEntry {
                            line: line_num,
                            content: line.trim().to_owned(),
                        });
                    }
                }
            }

            if sites.is_empty() {
                None
            } else {
                Some(CallerFile { path: relative, sites })
            }
        })
        .collect();

    let mut results = results;
    results.sort_by(|a, b| a.path.to_ascii_lowercase().cmp(&b.path.to_ascii_lowercase()));

    Ok(CallersOutput {
        declarations,
        files: results,
    })
}
