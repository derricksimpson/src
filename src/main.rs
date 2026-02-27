mod cli;
mod count;
mod exclusion;
mod file_reader;
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
use yaml_output::OutputFormat;

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

fn resolve_format(args: &cli::CliArgs) -> OutputFormat {
    match args.format {
        cli::OutputFormatArg::Json => OutputFormat::Json,
        cli::OutputFormatArg::Yaml => OutputFormat::Yaml,
    }
}

fn emit(envelope: &OutputEnvelope, format: OutputFormat, output_path: &Option<String>) {
    if let Some(ref path) = output_path {
        if let Err(e) = yaml_output::write_output_to(envelope, format, path) {
            eprintln!("Failed to write output to {}: {}", path, e);
        }
    } else {
        yaml_output::write_output(envelope, format);
    }
}

fn make_meta(elapsed: u128, timed_out: bool, scanned: usize, matched: usize, total: Option<usize>) -> MetaInfo {
    MetaInfo {
        elapsed_ms: elapsed,
        timeout: timed_out,
        files_scanned: scanned,
        files_matched: matched,
        files_errored: 0,
        total_matches: total,
    }
}

fn timeout_error(timed_out: bool) -> Option<String> {
    if timed_out { Some("Operation timed out".into()) } else { None }
}

fn apply_limit<T>(items: Vec<T>, limit: Option<usize>) -> Vec<T> {
    match limit {
        Some(n) if n < items.len() => items.into_iter().take(n).collect(),
        _ => items,
    }
}

fn collect_file_errors(entries: &[FileEntry]) -> Vec<String> {
    entries.iter()
        .filter_map(|e| e.error.as_ref().map(|err| format!("{}: {}", e.path, err)))
        .collect()
}

fn execute(args: cli::CliArgs) -> i32 {
    let root = Path::new(&args.root);
    let format = resolve_format(&args);

    if !root.is_dir() {
        let envelope = OutputEnvelope {
            error: Some(format!("Directory not found: {}", args.root)),
            ..Default::default()
        };
        emit(&envelope, format, &args.output);
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
        execute_lines(&args, root, &cancelled, start, format)
    } else if args.graph {
        execute_graph(&args, root, &filter, &cancelled, start, format)
    } else if args.symbols {
        execute_symbols(&args, root, &filter, &cancelled, start, format)
    } else if args.stats {
        execute_stats(&args, root, &filter, &cancelled, start, format)
    } else if args.count && args.find.is_some() {
        execute_count(&args, root, &filter, &cancelled, start, format)
    } else if let Some(ref find_pattern) = args.find {
        execute_search(&args, root, find_pattern, &filter, &cancelled, start, format)
    } else if !args.globs.is_empty() {
        execute_file_listing(&args, root, &filter, &cancelled, start, format)
    } else {
        execute_directory_hierarchy(&args, root, &filter, &cancelled, start, format)
    }
}

fn execute_directory_hierarchy(
    args: &cli::CliArgs,
    root: &Path,
    filter: &exclusion::ExclusionFilter,
    cancelled: &AtomicBool,
    start: Instant,
    format: OutputFormat,
) -> i32 {
    let tree = scanner::scan_directories(root, filter, cancelled);
    let elapsed = start.elapsed().as_millis();
    let timed_out = cancelled.load(Ordering::Relaxed);

    let envelope = OutputEnvelope {
        meta: Some(make_meta(elapsed, timed_out, 0, 0, None)),
        tree: Some(tree),
        error: timeout_error(timed_out),
        ..Default::default()
    };

    emit(&envelope, format, &args.output);
    if timed_out { 2 } else { 0 }
}

fn execute_file_listing(
    args: &cli::CliArgs,
    root: &Path,
    filter: &exclusion::ExclusionFilter,
    cancelled: &AtomicBool,
    start: Instant,
    format: OutputFormat,
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

    let total = entries.len();
    let entries = apply_limit(entries, args.limit);
    let matched = entries.len();

    let envelope = OutputEnvelope {
        meta: Some(make_meta(elapsed, timed_out, total, matched, None)),
        files: Some(entries),
        error: timeout_error(timed_out),
        ..Default::default()
    };

    emit(&envelope, format, &args.output);
    if timed_out { 2 } else { 0 }
}

fn execute_search(
    args: &cli::CliArgs,
    root: &Path,
    pattern: &str,
    filter: &exclusion::ExclusionFilter,
    cancelled: &AtomicBool,
    start: Instant,
    format: OutputFormat,
) -> i32 {
    let matcher = match Matcher::build(pattern, args.is_regex) {
        Ok(m) => m,
        Err(e) => {
            let envelope = OutputEnvelope {
                error: Some(e),
                ..Default::default()
            };
            emit(&envelope, format, &args.output);
            return 1;
        }
    };

    let globs = if args.globs.is_empty() {
        vec!["*.*".to_owned()]
    } else {
        args.globs.clone()
    };

    let candidate_files = scanner::find_files(root, &globs, filter, cancelled);
    let scanned = candidate_files.len();

    if cancelled.load(Ordering::Relaxed) {
        let elapsed = start.elapsed().as_millis();
        let envelope = OutputEnvelope {
            meta: Some(make_meta(elapsed, true, scanned, 0, None)),
            error: Some("Operation timed out — partial results may be incomplete".into()),
            ..Default::default()
        };
        emit(&envelope, format, &args.output);
        return 2;
    }

    let entries = searcher::search_files(&candidate_files, root, &matcher, args.line_numbers, cancelled);
    let elapsed = start.elapsed().as_millis();
    let timed_out = cancelled.load(Ordering::Relaxed);

    let file_errors = collect_file_errors(&entries);
    let errored = file_errors.len();
    let entries = apply_limit(entries, args.limit);
    let matched = entries.len();

    let mut meta = make_meta(elapsed, timed_out, scanned, matched, None);
    meta.files_errored = errored;

    let envelope = OutputEnvelope {
        meta: Some(meta),
        files: Some(entries),
        errors: if file_errors.is_empty() { None } else { Some(file_errors) },
        error: if timed_out { Some("Operation timed out — partial results may be incomplete".into()) } else { None },
        ..Default::default()
    };

    emit(&envelope, format, &args.output);
    if timed_out { 2 } else { 0 }
}

fn execute_lines(
    args: &cli::CliArgs,
    root: &Path,
    cancelled: &AtomicBool,
    start: Instant,
    format: OutputFormat,
) -> i32 {
    let specs = match lines::parse_line_specs(&args.lines, root) {
        Ok(s) => s,
        Err(e) => {
            let envelope = OutputEnvelope {
                error: Some(e),
                ..Default::default()
            };
            emit(&envelope, format, &args.output);
            return 1;
        }
    };

    let entries = lines::extract_lines(&specs, root, args.line_numbers, cancelled);
    let elapsed = start.elapsed().as_millis();
    let timed_out = cancelled.load(Ordering::Relaxed);

    let file_errors = collect_file_errors(&entries);
    let errored = file_errors.len();
    let entries = apply_limit(entries, args.limit);
    let matched = entries.len();

    let mut meta = make_meta(elapsed, timed_out, 0, matched, None);
    meta.files_errored = errored;

    let envelope = OutputEnvelope {
        meta: Some(meta),
        files: Some(entries),
        errors: if file_errors.is_empty() { None } else { Some(file_errors) },
        error: timeout_error(timed_out),
        ..Default::default()
    };

    emit(&envelope, format, &args.output);
    if timed_out { 2 } else { 0 }
}

fn execute_graph(
    args: &cli::CliArgs,
    root: &Path,
    filter: &exclusion::ExclusionFilter,
    cancelled: &AtomicBool,
    start: Instant,
    format: OutputFormat,
) -> i32 {
    let files = if args.globs.is_empty() {
        scanner::find_files(root, &["*.*".to_owned()], filter, cancelled)
    } else {
        scanner::find_files(root, &args.globs, filter, cancelled)
    };

    let scanned = files.len();
    if cancelled.load(Ordering::Relaxed) {
        let elapsed = start.elapsed().as_millis();
        let envelope = OutputEnvelope {
            meta: Some(make_meta(elapsed, true, scanned, 0, None)),
            error: Some("Operation timed out".into()),
            ..Default::default()
        };
        emit(&envelope, format, &args.output);
        return 2;
    }

    let graph_entries = graph::build_graph(&files, root, cancelled);
    let elapsed = start.elapsed().as_millis();
    let timed_out = cancelled.load(Ordering::Relaxed);

    let graph_entries = apply_limit(graph_entries, args.limit);
    let matched = graph_entries.len();

    let envelope = OutputEnvelope {
        meta: Some(make_meta(elapsed, timed_out, scanned, matched, None)),
        graph: Some(graph_entries),
        error: timeout_error(timed_out),
        ..Default::default()
    };

    emit(&envelope, format, &args.output);
    if timed_out { 2 } else { 0 }
}

fn execute_symbols(
    args: &cli::CliArgs,
    root: &Path,
    filter: &exclusion::ExclusionFilter,
    cancelled: &AtomicBool,
    start: Instant,
    format: OutputFormat,
) -> i32 {
    let files = if args.globs.is_empty() {
        scanner::find_files(root, &["*.*".to_owned()], filter, cancelled)
    } else {
        scanner::find_files(root, &args.globs, filter, cancelled)
    };

    let scanned = files.len();
    if cancelled.load(Ordering::Relaxed) {
        let elapsed = start.elapsed().as_millis();
        let envelope = OutputEnvelope {
            meta: Some(make_meta(elapsed, true, scanned, 0, None)),
            error: Some("Operation timed out".into()),
            ..Default::default()
        };
        emit(&envelope, format, &args.output);
        return 2;
    }

    let symbol_files = symbols::extract_symbols(&files, root, cancelled);
    let elapsed = start.elapsed().as_millis();
    let timed_out = cancelled.load(Ordering::Relaxed);

    let sym_errors: Vec<String> = symbol_files.iter()
        .filter_map(|sf| sf.error.as_ref().map(|err| format!("{}: {}", sf.path, err)))
        .collect();
    let errored = sym_errors.len();
    let symbol_files = apply_limit(symbol_files, args.limit);
    let matched = symbol_files.len();

    let mut meta = make_meta(elapsed, timed_out, scanned, matched, None);
    meta.files_errored = errored;

    let envelope = OutputEnvelope {
        meta: Some(meta),
        symbols: Some(symbol_files),
        errors: if sym_errors.is_empty() { None } else { Some(sym_errors) },
        error: timeout_error(timed_out),
        ..Default::default()
    };

    emit(&envelope, format, &args.output);
    if timed_out { 2 } else { 0 }
}

fn execute_stats(
    args: &cli::CliArgs,
    root: &Path,
    filter: &exclusion::ExclusionFilter,
    cancelled: &AtomicBool,
    start: Instant,
    format: OutputFormat,
) -> i32 {
    let files = if args.globs.is_empty() {
        scanner::find_files(root, &["*.*".to_owned()], filter, cancelled)
    } else {
        scanner::find_files(root, &args.globs, filter, cancelled)
    };

    let scanned = files.len();
    if cancelled.load(Ordering::Relaxed) {
        let elapsed = start.elapsed().as_millis();
        let envelope = OutputEnvelope {
            meta: Some(make_meta(elapsed, true, scanned, 0, None)),
            error: Some("Operation timed out".into()),
            ..Default::default()
        };
        emit(&envelope, format, &args.output);
        return 2;
    }

    let stats_output = stats::compute_stats(&files, root, cancelled);
    let elapsed = start.elapsed().as_millis();
    let timed_out = cancelled.load(Ordering::Relaxed);

    let envelope = OutputEnvelope {
        meta: Some(make_meta(elapsed, timed_out, scanned, scanned, None)),
        stats: Some(stats_output),
        error: timeout_error(timed_out),
        ..Default::default()
    };

    emit(&envelope, format, &args.output);
    if timed_out { 2 } else { 0 }
}

fn execute_count(
    args: &cli::CliArgs,
    root: &Path,
    filter: &exclusion::ExclusionFilter,
    cancelled: &AtomicBool,
    start: Instant,
    format: OutputFormat,
) -> i32 {
    let pattern = args.find.as_ref().unwrap();
    let matcher = match Matcher::build(pattern, args.is_regex) {
        Ok(m) => m,
        Err(e) => {
            let envelope = OutputEnvelope {
                error: Some(e),
                ..Default::default()
            };
            emit(&envelope, format, &args.output);
            return 1;
        }
    };

    let globs = if args.globs.is_empty() {
        vec!["*.*".to_owned()]
    } else {
        args.globs.clone()
    };

    let candidate_files = scanner::find_files(root, &globs, filter, cancelled);
    let scanned = candidate_files.len();

    if cancelled.load(Ordering::Relaxed) {
        let elapsed = start.elapsed().as_millis();
        let envelope = OutputEnvelope {
            meta: Some(make_meta(elapsed, true, scanned, 0, Some(0))),
            error: Some("Operation timed out".into()),
            ..Default::default()
        };
        emit(&envelope, format, &args.output);
        return 2;
    }

    let (count_entries, total) = count::count_matches(&candidate_files, root, &matcher, cancelled);
    let elapsed = start.elapsed().as_millis();
    let timed_out = cancelled.load(Ordering::Relaxed);

    let count_entries = apply_limit(count_entries, args.limit);
    let matched = count_entries.len();

    let envelope = OutputEnvelope {
        meta: Some(make_meta(elapsed, timed_out, scanned, matched, Some(total))),
        counts: Some(count_entries),
        error: if timed_out { Some("Operation timed out — partial results may be incomplete".into()) } else { None },
        ..Default::default()
    };

    emit(&envelope, format, &args.output);
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
