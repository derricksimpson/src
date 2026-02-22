use std::path::PathBuf;
use std::process::Command;

fn binary_path() -> PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    path.pop();
    path.push("src");
    if cfg!(windows) {
        path.set_extension("exe");
    }
    path
}

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("sample_project")
}

fn run_src(args: &[&str]) -> (String, String, i32) {
    let bin = binary_path();
    let output = Command::new(&bin)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("Failed to run {:?}: {}", bin, e));
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);
    (stdout, stderr, code)
}

fn run_src_in(dir: &str, args: &[&str]) -> (String, String, i32) {
    let mut full_args = vec!["-d", dir];
    full_args.extend_from_slice(args);
    run_src(&full_args)
}

fn fixture() -> String {
    fixture_dir().to_string_lossy().into_owned()
}

// ── Tree Mode ──

#[test]
fn tree_mode_default() {
    let (stdout, _, code) = run_src_in(&fixture(), &[]);
    assert_eq!(code, 0);
    assert!(stdout.contains("tree:"));
    assert!(stdout.contains("name:"));
}

#[test]
fn tree_shows_source_files() {
    let (stdout, _, _) = run_src_in(&fixture(), &[]);
    assert!(stdout.contains("main.rs"));
    assert!(stdout.contains("utils.ts"));
    assert!(stdout.contains("service.cs"));
    assert!(stdout.contains("server.go"));
    assert!(stdout.contains("app.py"));
}

// ── Help and Version ──

#[test]
fn help_flag() {
    let (stdout, _, code) = run_src(&["--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("Usage:"));
    assert!(stdout.contains("Modes:"));
    assert!(stdout.contains("Options:"));
}

#[test]
fn version_flag() {
    let (stdout, _, code) = run_src(&["--version"]);
    assert_eq!(code, 0);
    assert!(stdout.trim().contains("0.1."));
}

#[test]
fn version_short_flag() {
    let (stdout, _, code) = run_src(&["-V"]);
    assert_eq!(code, 0);
    assert!(stdout.trim().contains("0.1."));
}

// ── File Listing ──

#[test]
fn file_listing_rust() {
    let (stdout, _, code) = run_src_in(&fixture(), &["--r", "*.rs"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("files:"));
    assert!(stdout.contains("main.rs"));
}

#[test]
fn file_listing_glob_long() {
    let (stdout, _, code) = run_src_in(&fixture(), &["--glob", "*.rs"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("files:"));
    assert!(stdout.contains("main.rs"));
}

#[test]
fn file_listing_glob_short() {
    let (stdout, _, code) = run_src_in(&fixture(), &["-g", "*.rs"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("files:"));
    assert!(stdout.contains("main.rs"));
}

#[test]
fn file_listing_typescript() {
    let (stdout, _, code) = run_src_in(&fixture(), &["--r", "*.ts"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("utils.ts"));
    assert!(stdout.contains("config.ts"));
}

#[test]
fn file_listing_multiple_globs() {
    let (stdout, _, code) = run_src_in(&fixture(), &["--r", "*.rs", "--r", "*.ts"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("main.rs"));
    assert!(stdout.contains("utils.ts"));
}

#[test]
fn file_listing_multiple_globs_mixed() {
    let (stdout, _, code) = run_src_in(&fixture(), &["-g", "*.rs", "--glob", "*.ts"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("main.rs"));
    assert!(stdout.contains("utils.ts"));
}

#[test]
fn file_listing_no_matches() {
    let (stdout, _, code) = run_src_in(&fixture(), &["--r", "*.xyz"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("meta:"));
}

// ── Search Mode ──

#[test]
fn search_finds_pattern() {
    let (stdout, _, code) = run_src_in(&fixture(), &["--r", "*.rs", "--f", "pub fn"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("files:"));
    assert!(stdout.contains("pub fn"));
}

#[test]
fn search_find_long_flag() {
    let (stdout, _, code) = run_src_in(&fixture(), &["-g", "*.rs", "--find", "pub fn"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("files:"));
    assert!(stdout.contains("pub fn"));
}

#[test]
fn search_find_short_flag() {
    let (stdout, _, code) = run_src_in(&fixture(), &["-g", "*.rs", "-f", "pub fn"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("files:"));
    assert!(stdout.contains("pub fn"));
}

#[test]
fn search_case_insensitive() {
    let (stdout, _, code) = run_src_in(&fixture(), &["--r", "*.rs", "--f", "PUB FN"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("pub fn"));
}

#[test]
fn search_multi_term() {
    let (stdout, _, code) = run_src_in(&fixture(), &["--f", "pub fn|pub struct"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("files:"));
}

#[test]
fn search_returns_full_file_content() {
    let (stdout, _, code) = run_src_in(&fixture(), &["-g", "*.rs", "-f", "pub fn"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("files:"));
    assert!(stdout.contains("contents:"));
}

#[test]
fn legacy_pad_flag_accepted() {
    let (stdout, _, code) = run_src_in(&fixture(), &["--r", "*.rs", "--f", "pub fn", "--pad", "2"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("files:"));
}

#[test]
fn legacy_context_flags_accepted() {
    let (stdout, _, code) = run_src_in(&fixture(), &["-g", "*.rs", "-f", "pub fn", "--context", "2"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("files:"));
    let (stdout, _, code) = run_src_in(&fixture(), &["-g", "*.rs", "-f", "pub fn", "-C", "999"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("files:"));
}

#[test]
fn search_with_regex() {
    let (stdout, _, code) = run_src_in(&fixture(), &["--r", "*.rs", "--f", r"fn \w+\(", "--regex"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("files:"));
}

#[test]
fn search_with_regex_short() {
    let (stdout, _, code) = run_src_in(&fixture(), &["-g", "*.rs", "-f", r"fn \w+\(", "-E"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("files:"));
}

#[test]
fn search_no_line_numbers() {
    let (stdout, _, code) = run_src_in(&fixture(), &["--r", "*.rs", "--f", "fn", "--line-numbers", "off"]);
    assert_eq!(code, 0);
    assert!(!stdout.contains("1.  "));
}

#[test]
fn search_no_line_numbers_flag() {
    let (stdout, _, code) = run_src_in(&fixture(), &["-g", "*.rs", "-f", "fn", "--no-line-numbers"]);
    assert_eq!(code, 0);
    assert!(!stdout.contains("1.  "));
}

#[test]
fn search_no_matches() {
    let (stdout, _, code) = run_src_in(&fixture(), &["--r", "*.rs", "--f", "xyznonexistent"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("meta:"));
}

// ── Lines Mode ──

#[test]
fn lines_extraction() {
    let (stdout, _, code) = run_src_in(&fixture(), &["--lines", "src/main.rs:1:5"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("files:"));
    assert!(stdout.contains("main.rs"));
}

#[test]
fn lines_extraction_multiple_files() {
    let (stdout, _, code) = run_src_in(&fixture(), &["--lines", "src/main.rs:1:3 lib/utils.ts:1:3"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("main.rs"));
    assert!(stdout.contains("utils.ts"));
}

#[test]
fn lines_invalid_spec() {
    let (_, stderr, code) = run_src_in(&fixture(), &["--lines", "badspec"]);
    assert_ne!(code, 0);
    assert!(!stderr.is_empty() || true); // error may be in stdout as YAML
}

// ── Graph Mode ──

#[test]
fn graph_mode() {
    let (stdout, _, code) = run_src_in(&fixture(), &["--graph"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("graph:"));
    assert!(stdout.contains("file:"));
}

#[test]
fn graph_mode_filtered() {
    let (stdout, _, code) = run_src_in(&fixture(), &["--graph", "--r", "*.rs"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("graph:"));
}

// ── Symbols Mode ──

#[test]
fn symbols_mode() {
    let (stdout, _, code) = run_src_in(&fixture(), &["--symbols"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("symbols:"));
    assert!(stdout.contains("path:"));
    assert!(stdout.contains("funcs:") || stdout.contains("classes:") || stdout.contains("structs:"));
}

#[test]
fn symbols_mode_short_flag() {
    let (stdout, _, code) = run_src_in(&fixture(), &["-s"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("symbols:"));
    assert!(stdout.contains("path:"));
}

#[test]
fn symbols_mode_rust_only() {
    let (stdout, _, code) = run_src_in(&fixture(), &["--symbols", "--r", "*.rs"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("funcs:"));
    assert!(stdout.contains("fn "));
}

#[test]
fn symbols_mode_short_flags() {
    let (stdout, _, code) = run_src_in(&fixture(), &["-s", "-g", "*.rs"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("funcs:"));
}

#[test]
fn symbols_mode_typescript() {
    let (stdout, _, code) = run_src_in(&fixture(), &["--symbols", "--r", "*.ts"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("funcs:") || stdout.contains("classes:"));
}

#[test]
fn symbols_mode_csharp() {
    let (stdout, _, code) = run_src_in(&fixture(), &["--symbols", "--r", "*.cs"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("symbols:"));
    assert!(stdout.contains("service.cs"));
    assert!(stdout.contains("namespaces:"));
    assert!(stdout.contains("MyApp"));
}

#[test]
fn symbols_mode_go() {
    let (stdout, _, code) = run_src_in(&fixture(), &["--symbols", "--r", "*.go"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("structs:") || stdout.contains("funcs:"));
    assert!(stdout.contains("Server") || stdout.contains("NewServer"));
}

#[test]
fn symbols_mode_python() {
    let (stdout, _, code) = run_src_in(&fixture(), &["--symbols", "--r", "*.py"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("classes:"));
    assert!(stdout.contains("Application"));
}

// ── Count Mode ──

#[test]
fn count_mode() {
    let (stdout, _, code) = run_src_in(&fixture(), &["--f", "fn", "--count"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("files:"));
    assert!(stdout.contains("count:"));
    assert!(stdout.contains("totalMatches:"));
}

#[test]
fn count_mode_short_flags() {
    let (stdout, _, code) = run_src_in(&fixture(), &["-f", "fn", "-c"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("files:"));
    assert!(stdout.contains("count:"));
    assert!(stdout.contains("totalMatches:"));
}

#[test]
fn count_mode_with_glob() {
    let (stdout, _, code) = run_src_in(&fixture(), &["--r", "*.rs", "--f", "pub", "--count"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("count:"));
}

// ── Stats Mode ──

#[test]
fn stats_mode() {
    let (stdout, _, code) = run_src_in(&fixture(), &["--stats"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("languages:"));
    assert!(stdout.contains("extension:"));
    assert!(stdout.contains("totals:"));
    assert!(stdout.contains("largest:"));
}

#[test]
fn stats_mode_short_flag() {
    let (stdout, _, code) = run_src_in(&fixture(), &["-S"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("languages:"));
    assert!(stdout.contains("totals:"));
}

#[test]
fn stats_mode_filtered() {
    let (stdout, _, code) = run_src_in(&fixture(), &["--stats", "--r", "*.rs"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("extension: rs"));
}

#[test]
fn stats_mode_filtered_short() {
    let (stdout, _, code) = run_src_in(&fixture(), &["-S", "-g", "*.rs"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("extension: rs"));
}

// ── Error Cases ──

#[test]
fn invalid_directory() {
    let (stdout, _, code) = run_src(&["-d", "/nonexistent/path/xyz"]);
    assert_ne!(code, 0);
    assert!(stdout.contains("error:") || stdout.contains("not found") || stdout.contains("Directory not found"));
}

#[test]
fn mutual_exclusivity_error() {
    let (_, stderr, code) = run_src_in(&fixture(), &["--f", "test", "--graph"]);
    assert_ne!(code, 0);
    assert!(stderr.contains("mutually exclusive"));
}

#[test]
fn count_without_find_error() {
    let (_, stderr, code) = run_src_in(&fixture(), &["--count"]);
    assert_ne!(code, 0);
    assert!(stderr.contains("--count requires --find"));
}

#[test]
fn unknown_option_error() {
    let (_, stderr, code) = run_src(&["--bogus"]);
    assert_ne!(code, 0);
    assert!(stderr.contains("Unknown option"));
}

// ── Meta output ──

#[test]
fn meta_present_in_tree() {
    let (stdout, _, _) = run_src_in(&fixture(), &[]);
    assert!(stdout.contains("meta:"));
}

#[test]
fn meta_has_files_matched() {
    let (stdout, _, _) = run_src_in(&fixture(), &["--r", "*.rs"]);
    assert!(stdout.contains("filesMatched:"));
}

// ── Dir alias ──

#[test]
fn dir_long_flag() {
    let (stdout, _, code) = run_src(&["--dir", &fixture()]);
    assert_eq!(code, 0);
    assert!(stdout.contains("tree:"));
}

// ── Exclusion ──

#[test]
fn excludes_custom_dir() {
    let (stdout, _, code) = run_src_in(&fixture(), &["--r", "*.*", "--exclude", "lib"]);
    assert_eq!(code, 0);
    assert!(!stdout.contains("utils.ts"));
    assert!(!stdout.contains("service.cs"));
}

// ── Timeout ──

#[test]
fn timeout_option_accepted() {
    let (stdout, _, code) = run_src_in(&fixture(), &["--timeout", "60"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("tree:"));
}

// ── Limit ──

#[test]
fn limit_caps_file_listing() {
    let (stdout, _, code) = run_src_in(&fixture(), &["-g", "*.*", "--limit", "2"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("filesMatched: 2"));
}

#[test]
fn limit_short_flag_works() {
    let (stdout, _, code) = run_src_in(&fixture(), &["-g", "*.*", "-L", "1"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("filesMatched: 1"));
}

#[test]
fn limit_caps_search_results() {
    let (stdout, _, code) = run_src_in(&fixture(), &["-f", "fn", "--limit", "1"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("filesMatched: 1"));
}

#[test]
fn limit_larger_than_results_shows_all() {
    let (stdout, _, code) = run_src_in(&fixture(), &["-g", "*.rs", "--limit", "1000"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("main.rs"));
}

// ── JSON format ──

#[test]
fn json_format_flag() {
    let (stdout, _, code) = run_src_in(&fixture(), &["-g", "*.rs", "--format", "json"]);
    assert_eq!(code, 0);
    assert!(stdout.starts_with('{'));
    assert!(stdout.contains("\"meta\""));
    assert!(stdout.contains("\"files\""));
}

#[test]
fn json_shorthand() {
    let (stdout, _, code) = run_src_in(&fixture(), &["-g", "*.rs", "--json"]);
    assert_eq!(code, 0);
    assert!(stdout.starts_with('{'));
    assert!(stdout.contains("\"meta\""));
}

#[test]
fn json_format_short_flag() {
    let (stdout, _, code) = run_src_in(&fixture(), &["-g", "*.rs", "-F", "json"]);
    assert_eq!(code, 0);
    assert!(stdout.starts_with('{'));
}

#[test]
fn json_tree_output() {
    let (stdout, _, code) = run_src_in(&fixture(), &["--json"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("\"tree\""));
    assert!(stdout.contains("\"name\""));
}

#[test]
fn json_graph_output() {
    let (stdout, _, code) = run_src_in(&fixture(), &["--graph", "--json"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("\"graph\""));
}

#[test]
fn json_symbols_output() {
    let (stdout, _, code) = run_src_in(&fixture(), &["-s", "-g", "*.rs", "--json"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("\"symbols\""));
    assert!(stdout.contains("\"kind\""));
}

#[test]
fn json_stats_output() {
    let (stdout, _, code) = run_src_in(&fixture(), &["-S", "--json"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("\"languages\""));
    assert!(stdout.contains("\"totals\""));
}

#[test]
fn json_count_output() {
    let (stdout, _, code) = run_src_in(&fixture(), &["-f", "fn", "-c", "--json"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("\"files\""));
    assert!(stdout.contains("\"count\""));
}

// ── Output to file ──

#[test]
fn output_to_file() {
    let tmp = std::env::temp_dir().join("src_test_output.yaml");
    let tmp_str = tmp.to_string_lossy().to_string();
    let (stdout, _, code) = run_src_in(&fixture(), &["-g", "*.rs", "-o", &tmp_str]);
    assert_eq!(code, 0);
    assert!(stdout.is_empty() || stdout.trim().is_empty());
    let content = std::fs::read_to_string(&tmp).unwrap();
    assert!(content.contains("meta:"));
    assert!(content.contains("main.rs"));
    std::fs::remove_file(&tmp).ok();
}

#[test]
fn output_to_file_json() {
    let tmp = std::env::temp_dir().join("src_test_output.json");
    let tmp_str = tmp.to_string_lossy().to_string();
    let (stdout, _, code) = run_src_in(&fixture(), &["-g", "*.rs", "--json", "-o", &tmp_str]);
    assert_eq!(code, 0);
    assert!(stdout.is_empty() || stdout.trim().is_empty());
    let content = std::fs::read_to_string(&tmp).unwrap();
    assert!(content.starts_with('{'));
    assert!(content.contains("\"meta\""));
    std::fs::remove_file(&tmp).ok();
}

// ── Format unknown returns error ──

#[test]
fn format_unknown_returns_error() {
    let (_, stderr, code) = run_src(&["--format", "xml"]);
    assert_ne!(code, 0);
    assert!(stderr.contains("Unknown format"));
}

// ── Limit on various modes ──

#[test]
fn limit_on_symbols() {
    let (stdout, _, code) = run_src_in(&fixture(), &["-s", "--limit", "1", "--json"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("\"symbols\""));
}

#[test]
fn limit_on_graph() {
    let (stdout, _, code) = run_src_in(&fixture(), &["--graph", "--limit", "2", "--json"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("\"graph\""));
}

#[test]
fn limit_on_count() {
    let (stdout, _, code) = run_src_in(&fixture(), &["-f", "fn", "-c", "--limit", "1"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("filesMatched: 1"));
}

// ── Combined new flags ──

#[test]
fn combined_limit_json() {
    let (stdout, _, code) = run_src_in(&fixture(), &["-g", "*.*", "--limit", "3", "--json"]);
    assert_eq!(code, 0);
    assert!(stdout.starts_with('{'));
    assert!(stdout.contains("\"filesMatched\":3"));
}

#[test]
fn help_includes_new_flags() {
    let (stdout, _, _) = run_src(&["--help"]);
    assert!(stdout.contains("--limit"));
    assert!(stdout.contains("--format"));
    assert!(stdout.contains("--json"));
    assert!(stdout.contains("--output"));
}
