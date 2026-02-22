use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use rayon::prelude::*;

use crate::file_reader;
use crate::models::CountEntry;
use crate::path_helper;
use crate::searcher::Matcher;

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

    let content = match file_reader::read_file(path) {
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
