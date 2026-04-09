use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use rayon::prelude::*;

use crate::alias::{self, AliasMapping};
use crate::file_reader;
use crate::lang;
use crate::models::GraphEntry;
use crate::path_helper;

pub fn build_graph(
    file_paths: &[String],
    root: &Path,
    cancelled: &AtomicBool,
    aliases: &[AliasMapping],
) -> Vec<GraphEntry> {
    let project_files: HashSet<String> = file_paths
        .iter()
        .map(|f| path_helper::normalized_relative(root, Path::new(f)))
        .collect();

    let mut entries: Vec<GraphEntry> = file_paths
        .par_iter()
        .filter_map(|file_path| {
            if cancelled.load(Ordering::Relaxed) {
                return None;
            }
            process_file(file_path, root, &project_files, aliases)
        })
        .collect();

    entries.sort_unstable_by(|a, b| a.file.to_ascii_lowercase().cmp(&b.file.to_ascii_lowercase()));
    entries
}

fn process_file(
    file_path: &str,
    root: &Path,
    project_files: &HashSet<String>,
    aliases: &[AliasMapping],
) -> Option<GraphEntry> {
    let path = Path::new(file_path);
    let relative = path_helper::normalized_relative(root, path);

    let ext = path.extension()?.to_str()?;
    let handler = lang::get_handler(ext)?;

    let content = match file_reader::read_file(path) {
        Ok(Some(c)) => c,
        Ok(None) => return Some(GraphEntry {
            file: relative,
            imports: Vec::new(),
        }),
        Err(_) => return Some(GraphEntry {
            file: relative,
            imports: Vec::new(),
        }),
    };

    let rel_path = Path::new(&relative);
    let raw_imports = handler.extract_imports(&content, rel_path);

    let mut resolved: Vec<String> = Vec::new();
    let mut seen = HashSet::new();

    for candidate in &raw_imports {
        if let Some(specifier) = candidate.strip_prefix(alias::ALIAS_PREFIX) {
            let alias_candidates = alias::resolve_alias(specifier, aliases);
            for ac in &alias_candidates {
                let normalized = normalize_candidate(ac);
                if normalized == relative {
                    continue;
                }
                if project_files.contains(&normalized) && seen.insert(normalized.clone()) {
                    resolved.push(normalized);
                }
            }
        } else {
            let normalized = normalize_candidate(candidate);
            if normalized == relative {
                continue;
            }
            if normalized.ends_with('/') {
                for pf in project_files.iter() {
                    if pf.starts_with(&normalized) && seen.insert(pf.clone()) {
                        resolved.push(pf.clone());
                    }
                }
            } else if project_files.contains(&normalized) && seen.insert(normalized.clone()) {
                resolved.push(normalized);
            }
        }
    }

    resolved.sort_unstable_by(|a, b| a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase()));

    Some(GraphEntry {
        file: relative,
        imports: resolved,
    })
}

fn normalize_candidate(candidate: &str) -> String {
    let s = if cfg!(windows) {
        candidate.replace('\\', "/")
    } else {
        candidate.to_owned()
    };

    let parts: Vec<&str> = s.split('/').collect();
    let mut normalized: Vec<&str> = Vec::new();
    for part in &parts {
        match *part {
            "." => continue,
            ".." => { normalized.pop(); }
            _ => normalized.push(part),
        }
    }
    normalized.join("/")
}
