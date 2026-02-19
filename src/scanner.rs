use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use rayon::prelude::*;

use crate::exclusion::ExclusionFilter;
use crate::glob;
use crate::models::ScanResult;

const SOURCE_EXTENSIONS: &[&str] = &[
    "cs", "ts", "tsx", "js", "jsx", "py", "rb", "go", "rs",
    "java", "kt", "scala", "swift", "m", "mm", "c", "cpp",
    "cc", "cxx", "h", "hpp", "hxx", "lua", "pl", "pm",
    "php", "r", "dart", "ex", "exs", "erl", "hs", "fs",
    "fsx", "fsi", "ml", "mli", "v", "sv", "vhd", "vhdl",
    "sql", "sh", "bash", "zsh", "ps1", "psm1", "bat", "cmd",
    "yaml", "yml", "json", "xml", "html", "htm", "css",
    "scss", "sass", "less", "vue", "svelte", "astro",
    "md", "mdx", "rst", "txt", "toml", "ini", "cfg",
    "conf", "env", "dockerfile", "tf", "tfvars", "hcl",
    "proto", "graphql", "gql", "razor", "cshtml", "csproj",
    "sln", "gradle", "cmake", "makefile", "mk",
];

fn is_source_file(name: &str) -> bool {
    if let Some(dot_pos) = name.rfind('.') {
        let ext = &name[dot_pos + 1..];
        SOURCE_EXTENSIONS.iter().any(|&e| ext.eq_ignore_ascii_case(e))
    } else {
        let lower = name.to_ascii_lowercase();
        lower == "makefile" || lower == "dockerfile" || lower == "rakefile" || lower == "gemfile"
    }
}

pub fn scan_directories(
    root: &Path,
    filter: &ExclusionFilter,
    cancelled: &AtomicBool,
) -> ScanResult {
    if cancelled.load(Ordering::Relaxed) {
        return ScanResult { name: String::new(), children: None, files: None };
    }

    let dir_name = root.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.to_string_lossy().into_owned());

    let mut files = Vec::new();
    let mut subdirs = Vec::new();

    let entries = match fs::read_dir(root) {
        Ok(rd) => rd,
        Err(_) => return ScanResult { name: dir_name, children: None, files: None },
    };

    for entry in entries.flatten() {
        let ft = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if ft.is_file() {
            if is_source_file(&name_str) {
                files.push(name_str.into_owned());
            }
        } else if ft.is_dir() && !filter.is_excluded(&name_str) {
            subdirs.push(entry.path());
        }
    }

    let children: Vec<ScanResult> = subdirs
        .par_iter()
        .filter_map(|subdir| {
            if cancelled.load(Ordering::Relaxed) {
                return None;
            }
            let child = scan_directories(subdir, filter, cancelled);
            if child.files.is_some() || child.children.is_some() {
                Some(child)
            } else {
                None
            }
        })
        .collect();

    let mut children = children;
    children.sort_by(|a, b| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()));
    files.sort_by(|a: &String, b: &String| a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase()));

    ScanResult {
        name: dir_name,
        children: if children.is_empty() { None } else { Some(children) },
        files: if files.is_empty() { None } else { Some(files) },
    }
}

pub fn find_files(
    root: &Path,
    globs: &[String],
    filter: &ExclusionFilter,
    cancelled: &AtomicBool,
) -> Vec<String> {
    let mut results = Vec::new();
    find_files_recursive(root, globs, filter, cancelled, &mut results);
    results.sort_by(|a, b| a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase()));
    results
}

fn find_files_recursive(
    dir: &Path,
    globs: &[String],
    filter: &ExclusionFilter,
    cancelled: &AtomicBool,
    results: &mut Vec<String>,
) {
    if cancelled.load(Ordering::Relaxed) {
        return;
    }

    let entries = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return,
    };

    let mut subdirs = Vec::new();

    for entry in entries.flatten() {
        let ft = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if ft.is_file() {
            if glob::matches_any(&name_str, globs) {
                results.push(entry.path().to_string_lossy().into_owned());
            }
        } else if ft.is_dir() && !filter.is_excluded(&name_str) {
            subdirs.push(entry.path());
        }
    }

    let sub_results: Vec<Vec<String>> = subdirs
        .par_iter()
        .map(|subdir| {
            let mut sub = Vec::new();
            find_files_recursive(subdir, globs, filter, cancelled, &mut sub);
            sub
        })
        .collect();

    for sub in sub_results {
        results.extend(sub);
    }
}
