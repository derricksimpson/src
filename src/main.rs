mod cli;
mod count;
mod exclusion;
mod glob;
mod graph;
mod lang;
mod lines;
mod models;
mod path_helper;
mod scanner;
mod searcher;
mod stats;
mod symbols;
mod yaml_output;

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use models::{FileEntry, MetaInfo, OutputEnvelope};
use searcher::Matcher;

fn main() {
    let exit_code = run();
    std::process::exit(exit_code);
}

fn run() -> i32 {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let action = match cli::parse_args(&args) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{}", e);
            return 1;
        }
    };

    match action {
        cli::CliAction::Help => {
            cli::print_help();
            0
        }
        cli::CliAction::Version => {
            println!("0.1.0");
            0
        }
        cli::CliAction::Run(args) => execute(args),
    }
}

fn execute(args: cli::CliArgs) -> i32 {
    let root = Path::new(&args.root);
    if !root.is_dir() {
        let envelope = OutputEnvelope {
            meta: None,
            files: None,
            tree: None,
            graph: None,
            symbols: None,
            counts: None,
            stats: None,
            error: Some(format!("Directory not found: {}", args.root)),
        };
        yaml_output::write_output(&envelope);
        return 1;
    }

    let cancelled = Arc::new(AtomicBool::new(false));

    {
        let cancelled = cancelled.clone();
        ctrlc_handler(cancelled);
    }

    if let Some(secs) = args.timeout {
        let cancelled = cancelled.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(secs));
            cancelled.store(true, Ordering::Relaxed);
        });
    }

    let filter = exclusion::ExclusionFilter::new(&args.excludes, args.no_defaults);
    let start = Instant::now();

    if !args.lines.is_empty() {
        execute_lines(&args, root, &cancelled, start)
    } else if args.graph {
        execute_graph(&args, root, &filter, &cancelled, start)
    } else if args.symbols {
        execute_symbols(&args, root, &filter, &cancelled, start)
    } else if args.stats {
        execute_stats(&args, root, &filter, &cancelled, start)
    } else if args.count && args.find.is_some() {
        execute_count(&args, root, &filter, &cancelled, start)
    } else if let Some(ref find_pattern) = args.find {
        execute_search(&args, root, find_pattern, &filter, &cancelled, start)
    } else if !args.globs.is_empty() {
        execute_file_listing(&args, root, &filter, &cancelled, start)
    } else {
        execute_directory_hierarchy(root, &filter, &cancelled, start)
    }
}

fn execute_directory_hierarchy(
    root: &Path,
    filter: &exclusion::ExclusionFilter,
    cancelled: &AtomicBool,
    start: Instant,
) -> i32 {
    let tree = scanner::scan_directories(root, filter, cancelled);
    let elapsed = start.elapsed().as_millis();
    let timed_out = cancelled.load(Ordering::Relaxed);

    let envelope = OutputEnvelope {
        meta: Some(MetaInfo {
            elapsed_ms: elapsed,
            timeout: timed_out,
            files_scanned: 0,
            files_matched: 0,
            total_matches: None,
        }),
        files: None,
        tree: Some(tree),
        graph: None,
        symbols: None,
        counts: None,
        stats: None,
        error: if timed_out { Some("Operation timed out".into()) } else { None },
    };

    yaml_output::write_output(&envelope);
    if timed_out { 2 } else { 0 }
}

fn execute_file_listing(
    args: &cli::CliArgs,
    root: &Path,
    filter: &exclusion::ExclusionFilter,
    cancelled: &AtomicBool,
    start: Instant,
) -> i32 {
    let files = scanner::find_files(root, &args.globs, filter, cancelled);
    let elapsed = start.elapsed().as_millis();
    let timed_out = cancelled.load(Ordering::Relaxed);

    let entries: Vec<FileEntry> = files
        .iter()
        .map(|f| FileEntry {
            path: path_helper::normalized_relative(root, Path::new(f)),
            contents: None,
            error: None,
            chunks: None,
        })
        .collect();

    let count = entries.len();
    let envelope = OutputEnvelope {
        meta: Some(MetaInfo {
            elapsed_ms: elapsed,
            timeout: timed_out,
            files_scanned: count,
            files_matched: count,
            total_matches: None,
        }),
        files: Some(entries),
        tree: None,
        graph: None,
        symbols: None,
        counts: None,
        stats: None,
        error: if timed_out { Some("Operation timed out".into()) } else { None },
    };

    yaml_output::write_output(&envelope);
    if timed_out { 2 } else { 0 }
}

fn execute_search(
    args: &cli::CliArgs,
    root: &Path,
    pattern: &str,
    filter: &exclusion::ExclusionFilter,
    cancelled: &AtomicBool,
    start: Instant,
) -> i32 {
    let matcher = match Matcher::build(pattern, args.is_regex) {
        Ok(m) => m,
        Err(e) => {
            let envelope = OutputEnvelope {
                meta: None,
                files: None,
                tree: None,
                graph: None,
                symbols: None,
                counts: None,
                stats: None,
                error: Some(e),
            };
            yaml_output::write_output(&envelope);
            return 1;
        }
    };

    let globs = if args.globs.is_empty() {
        vec!["*.*".to_owned()]
    } else {
        args.globs.clone()
    };

    let candidate_files = scanner::find_files(root, &globs, filter, cancelled);

    if cancelled.load(Ordering::Relaxed) {
        let elapsed = start.elapsed().as_millis();
        let envelope = OutputEnvelope {
            meta: Some(MetaInfo {
                elapsed_ms: elapsed,
                timeout: true,
                files_scanned: candidate_files.len(),
                files_matched: 0,
                total_matches: None,
            }),
            files: None,
            tree: None,
            graph: None,
            symbols: None,
            counts: None,
            stats: None,
            error: Some("Operation timed out — partial results may be incomplete".into()),
        };
        yaml_output::write_output(&envelope);
        return 2;
    }

    let entries = searcher::search_files(&candidate_files, root, &matcher, args.pad, args.line_numbers, cancelled);
    let elapsed = start.elapsed().as_millis();
    let timed_out = cancelled.load(Ordering::Relaxed);

    let envelope = OutputEnvelope {
        meta: Some(MetaInfo {
            elapsed_ms: elapsed,
            timeout: timed_out,
            files_scanned: candidate_files.len(),
            files_matched: entries.len(),
            total_matches: None,
        }),
        files: Some(entries),
        tree: None,
        graph: None,
        symbols: None,
        counts: None,
        stats: None,
        error: if timed_out { Some("Operation timed out — partial results may be incomplete".into()) } else { None },
    };

    yaml_output::write_output(&envelope);
    if timed_out { 2 } else { 0 }
}

fn execute_lines(
    args: &cli::CliArgs,
    root: &Path,
    cancelled: &AtomicBool,
    start: Instant,
) -> i32 {
    let specs = match lines::parse_line_specs(&args.lines, root) {
        Ok(s) => s,
        Err(e) => {
            let envelope = OutputEnvelope {
                meta: None,
                files: None,
                tree: None,
                graph: None,
                symbols: None,
                counts: None,
                stats: None,
                error: Some(e),
            };
            yaml_output::write_output(&envelope);
            return 1;
        }
    };

    let entries = lines::extract_lines(&specs, root, args.line_numbers, cancelled);
    let elapsed = start.elapsed().as_millis();
    let timed_out = cancelled.load(Ordering::Relaxed);

    let envelope = OutputEnvelope {
        meta: Some(MetaInfo {
            elapsed_ms: elapsed,
            timeout: timed_out,
            files_scanned: 0,
            files_matched: entries.len(),
            total_matches: None,
        }),
        files: Some(entries),
        tree: None,
        graph: None,
        symbols: None,
        counts: None,
        stats: None,
        error: if timed_out { Some("Operation timed out".into()) } else { None },
    };

    yaml_output::write_output(&envelope);
    if timed_out { 2 } else { 0 }
}

fn execute_graph(
    args: &cli::CliArgs,
    root: &Path,
    filter: &exclusion::ExclusionFilter,
    cancelled: &AtomicBool,
    start: Instant,
) -> i32 {
    let files = if args.globs.is_empty() {
        scanner::find_files(root, &["*.*".to_owned()], filter, cancelled)
    } else {
        scanner::find_files(root, &args.globs, filter, cancelled)
    };

    if cancelled.load(Ordering::Relaxed) {
        let elapsed = start.elapsed().as_millis();
        let envelope = OutputEnvelope {
            meta: Some(MetaInfo {
                elapsed_ms: elapsed,
                timeout: true,
                files_scanned: files.len(),
                files_matched: 0,
                total_matches: None,
            }),
            files: None,
            tree: None,
            graph: None,
            symbols: None,
            counts: None,
            stats: None,
            error: Some("Operation timed out".into()),
        };
        yaml_output::write_output(&envelope);
        return 2;
    }

    let graph_entries = graph::build_graph(&files, root, cancelled);
    let elapsed = start.elapsed().as_millis();
    let timed_out = cancelled.load(Ordering::Relaxed);

    let envelope = OutputEnvelope {
        meta: Some(MetaInfo {
            elapsed_ms: elapsed,
            timeout: timed_out,
            files_scanned: files.len(),
            files_matched: graph_entries.len(),
            total_matches: None,
        }),
        files: None,
        tree: None,
        graph: Some(graph_entries),
        symbols: None,
        counts: None,
        stats: None,
        error: if timed_out { Some("Operation timed out".into()) } else { None },
    };

    yaml_output::write_output(&envelope);
    if timed_out { 2 } else { 0 }
}

fn execute_symbols(
    args: &cli::CliArgs,
    root: &Path,
    filter: &exclusion::ExclusionFilter,
    cancelled: &AtomicBool,
    start: Instant,
) -> i32 {
    let files = if args.globs.is_empty() {
        scanner::find_files(root, &["*.*".to_owned()], filter, cancelled)
    } else {
        scanner::find_files(root, &args.globs, filter, cancelled)
    };

    if cancelled.load(Ordering::Relaxed) {
        let elapsed = start.elapsed().as_millis();
        let envelope = OutputEnvelope {
            meta: Some(MetaInfo {
                elapsed_ms: elapsed,
                timeout: true,
                files_scanned: files.len(),
                files_matched: 0,
                total_matches: None,
            }),
            files: None,
            tree: None,
            graph: None,
            symbols: None,
            counts: None,
            stats: None,
            error: Some("Operation timed out".into()),
        };
        yaml_output::write_output(&envelope);
        return 2;
    }

    let symbol_files = symbols::extract_symbols(&files, root, cancelled);
    let elapsed = start.elapsed().as_millis();
    let timed_out = cancelled.load(Ordering::Relaxed);

    let envelope = OutputEnvelope {
        meta: Some(MetaInfo {
            elapsed_ms: elapsed,
            timeout: timed_out,
            files_scanned: files.len(),
            files_matched: symbol_files.len(),
            total_matches: None,
        }),
        files: None,
        tree: None,
        graph: None,
        symbols: Some(symbol_files),
        counts: None,
        stats: None,
        error: if timed_out { Some("Operation timed out".into()) } else { None },
    };

    yaml_output::write_output(&envelope);
    if timed_out { 2 } else { 0 }
}

fn execute_stats(
    args: &cli::CliArgs,
    root: &Path,
    filter: &exclusion::ExclusionFilter,
    cancelled: &AtomicBool,
    start: Instant,
) -> i32 {
    let files = if args.globs.is_empty() {
        scanner::find_files(root, &["*.*".to_owned()], filter, cancelled)
    } else {
        scanner::find_files(root, &args.globs, filter, cancelled)
    };

    if cancelled.load(Ordering::Relaxed) {
        let elapsed = start.elapsed().as_millis();
        let envelope = OutputEnvelope {
            meta: Some(MetaInfo {
                elapsed_ms: elapsed,
                timeout: true,
                files_scanned: files.len(),
                files_matched: 0,
                total_matches: None,
            }),
            files: None,
            tree: None,
            graph: None,
            symbols: None,
            counts: None,
            stats: None,
            error: Some("Operation timed out".into()),
        };
        yaml_output::write_output(&envelope);
        return 2;
    }

    let stats_output = stats::compute_stats(&files, root, cancelled);
    let elapsed = start.elapsed().as_millis();
    let timed_out = cancelled.load(Ordering::Relaxed);

    let envelope = OutputEnvelope {
        meta: Some(MetaInfo {
            elapsed_ms: elapsed,
            timeout: timed_out,
            files_scanned: files.len(),
            files_matched: files.len(),
            total_matches: None,
        }),
        files: None,
        tree: None,
        graph: None,
        symbols: None,
        counts: None,
        stats: Some(stats_output),
        error: if timed_out { Some("Operation timed out".into()) } else { None },
    };

    yaml_output::write_output(&envelope);
    if timed_out { 2 } else { 0 }
}

fn execute_count(
    args: &cli::CliArgs,
    root: &Path,
    filter: &exclusion::ExclusionFilter,
    cancelled: &AtomicBool,
    start: Instant,
) -> i32 {
    let pattern = args.find.as_ref().unwrap();
    let matcher = match Matcher::build(pattern, args.is_regex) {
        Ok(m) => m,
        Err(e) => {
            let envelope = OutputEnvelope {
                meta: None,
                files: None,
                tree: None,
                graph: None,
                symbols: None,
                counts: None,
                stats: None,
                error: Some(e),
            };
            yaml_output::write_output(&envelope);
            return 1;
        }
    };

    let globs = if args.globs.is_empty() {
        vec!["*.*".to_owned()]
    } else {
        args.globs.clone()
    };

    let candidate_files = scanner::find_files(root, &globs, filter, cancelled);

    if cancelled.load(Ordering::Relaxed) {
        let elapsed = start.elapsed().as_millis();
        let envelope = OutputEnvelope {
            meta: Some(MetaInfo {
                elapsed_ms: elapsed,
                timeout: true,
                files_scanned: candidate_files.len(),
                files_matched: 0,
                total_matches: Some(0),
            }),
            files: None,
            tree: None,
            graph: None,
            symbols: None,
            counts: None,
            stats: None,
            error: Some("Operation timed out".into()),
        };
        yaml_output::write_output(&envelope);
        return 2;
    }

    let (count_entries, total) = count::count_matches(&candidate_files, root, &matcher, cancelled);
    let elapsed = start.elapsed().as_millis();
    let timed_out = cancelled.load(Ordering::Relaxed);

    let envelope = OutputEnvelope {
        meta: Some(MetaInfo {
            elapsed_ms: elapsed,
            timeout: timed_out,
            files_scanned: candidate_files.len(),
            files_matched: count_entries.len(),
            total_matches: Some(total),
        }),
        files: None,
        tree: None,
        graph: None,
        symbols: None,
        counts: Some(count_entries),
        stats: None,
        error: if timed_out { Some("Operation timed out — partial results may be incomplete".into()) } else { None },
    };

    yaml_output::write_output(&envelope);
    if timed_out { 2 } else { 0 }
}

#[cfg(unix)]
fn ctrlc_handler(cancelled: Arc<AtomicBool>) {
    unsafe {
        libc::signal(libc::SIGINT, handle_sigint as libc::sighandler_t);
    }
    CANCELLED_GLOBAL.store(Box::into_raw(Box::new(cancelled)) as usize, Ordering::SeqCst);
}

#[cfg(unix)]
static CANCELLED_GLOBAL: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[cfg(unix)]
extern "C" fn handle_sigint(_: libc::c_int) {
    let ptr = CANCELLED_GLOBAL.load(Ordering::SeqCst);
    if ptr != 0 {
        let cancelled = unsafe { &*(ptr as *const Arc<AtomicBool>) };
        cancelled.store(true, Ordering::SeqCst);
    }
}

#[cfg(windows)]
fn ctrlc_handler(cancelled: Arc<AtomicBool>) {
    use std::sync::OnceLock;
    static CANCELLED: OnceLock<Arc<AtomicBool>> = OnceLock::new();
    CANCELLED.get_or_init(|| cancelled);

    unsafe {
        SetConsoleCtrlHandler(Some(handler), 1);
    }

    unsafe extern "system" fn handler(_: u32) -> i32 {
        if let Some(c) = CANCELLED.get() {
            c.store(true, Ordering::SeqCst);
        }
        1
    }

    extern "system" {
        fn SetConsoleCtrlHandler(handler: Option<unsafe extern "system" fn(u32) -> i32>, add: i32) -> i32;
    }
}
