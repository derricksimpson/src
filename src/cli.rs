#[derive(Debug)]
pub struct CliArgs {
    pub root: String,
    pub globs: Vec<String>,
    pub find: Option<String>,
    pub pad: usize,
    pub timeout: Option<u64>,
    pub excludes: Vec<String>,
    pub no_defaults: bool,
    pub is_regex: bool,
    pub line_numbers: bool,
    pub lines: Vec<String>,
    pub graph: bool,
    pub symbols: bool,
    pub count: bool,
    pub stats: bool,
}

#[derive(Debug)]
pub enum CliAction {
    Run(CliArgs),
    Help,
    Version,
}

pub fn parse_args(args: &[String]) -> Result<CliAction, String> {
    let mut root: Option<String> = None;
    let mut globs = Vec::new();
    let mut find: Option<String> = None;
    let mut pad: usize = 0;
    let mut timeout: Option<u64> = None;
    let mut excludes = Vec::new();
    let mut no_defaults = false;
    let mut is_regex = false;
    let mut line_numbers = true;
    let mut lines: Vec<String> = Vec::new();
    let mut graph = false;
    let mut symbols = false;
    let mut count = false;
    let mut stats = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--root" | "-d" => {
                i += 1;
                if i >= args.len() { return Err("Missing value for --root".into()); }
                root = Some(args[i].clone());
            }
            "--r" => {
                i += 1;
                if i >= args.len() { return Err("Missing value for --r".into()); }
                globs.push(args[i].clone());
            }
            "--f" => {
                i += 1;
                if i >= args.len() { return Err("Missing value for --f".into()); }
                find = Some(args[i].clone());
            }
            "--pad" => {
                i += 1;
                if i >= args.len() { return Err("Missing value for --pad".into()); }
                pad = args[i].parse::<usize>()
                    .map_err(|_| format!("Invalid integer for --pad: {}", args[i]))?;
            }
            "--timeout" => {
                i += 1;
                if i >= args.len() { return Err("Missing value for --timeout".into()); }
                timeout = Some(args[i].parse::<u64>()
                    .map_err(|_| format!("Invalid integer for --timeout: {}", args[i]))?);
            }
            "--exclude" => {
                i += 1;
                if i >= args.len() { return Err("Missing value for --exclude".into()); }
                excludes.push(args[i].clone());
            }
            "--line-numbers" => {
                i += 1;
                if i >= args.len() { return Err("Missing value for --line-numbers".into()); }
                match args[i].as_str() {
                    "off" => line_numbers = false,
                    other => return Err(format!("Invalid value for --line-numbers: '{}'. Only 'off' is supported.", other)),
                }
            }
            "--lines" => {
                i += 1;
                if i >= args.len() { return Err("Missing value for --lines".into()); }
                for spec in args[i].split_whitespace() {
                    lines.push(spec.to_owned());
                }
            }
            "--graph" => graph = true,
            "--symbols" | "--s" => symbols = true,
            "--count" => count = true,
            "--stats" | "--st" => stats = true,
            "--no-defaults" => no_defaults = true,
            "--regex" => is_regex = true,
            "--help" | "-h" | "-?" => return Ok(CliAction::Help),
            "--version" => return Ok(CliAction::Version),
            other => return Err(format!("Unknown option: {}\nRun 'src --help' for usage information.", other)),
        }
        i += 1;
    }

    if count && find.is_none() {
        return Err("--count requires --f <pattern>".into());
    }

    let mut exclusive_count = 0;
    let mut exclusive_names = Vec::new();
    if find.is_some() && !count { exclusive_count += 1; exclusive_names.push("--f"); }
    if find.is_some() && count { exclusive_count += 1; exclusive_names.push("--f --count"); }
    if !lines.is_empty() { exclusive_count += 1; exclusive_names.push("--lines"); }
    if graph { exclusive_count += 1; exclusive_names.push("--graph"); }
    if symbols { exclusive_count += 1; exclusive_names.push("--symbols"); }
    if stats { exclusive_count += 1; exclusive_names.push("--stats"); }
    if exclusive_count > 1 {
        return Err(format!("{} are mutually exclusive and cannot be combined.", exclusive_names.join(" and ")));
    }

    let root = root.unwrap_or_else(|| std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| ".".into()));

    Ok(CliAction::Run(CliArgs {
        root,
        globs,
        find,
        pad,
        timeout,
        excludes,
        no_defaults,
        is_regex,
        line_numbers,
        lines,
        graph,
        symbols,
        count,
        stats,
    }))
}

pub fn print_help() {
    print!(
r#"src — fast source code interrogation tool

Usage:
  src [options]

Modes:
  (default)               Show directory hierarchy containing source files
  --r <glob>              List files matching glob patterns (repeatable)
  --f <pattern>           Search file contents for a pattern
  --lines "<specs>"       Extract specific line ranges from files
  --graph                 Show project-internal dependency graph
  --symbols, --s          Extract symbol declarations from source files
  --stats, --st           Show codebase statistics (files, lines, bytes by language)

Options:
  --root, -d <path>       Root directory (default: current directory)
  --r <glob>              File glob pattern (repeatable, e.g. --r *.ts --r *.cs)
  --f <pattern>           Search pattern (use | for OR, e.g. Payment|Invoice)
  --lines "<specs>"       Line specs: "file:start:end file2:start:end" (repeatable)
  --graph                 Emit source dependency graph
  --symbols, --s          Extract fn/struct/class/enum/trait declarations
  --count                 Show match counts per file (requires --f)
  --stats, --st           File counts, line counts, byte sizes by extension
  --pad <n>               Context lines before/after each match (default: 0)
  --line-numbers off      Suppress per-line number prefixes in content output
  --timeout <secs>        Max execution time in seconds
  --exclude <name>        Additional exclusions (repeatable)
  --no-defaults           Disable built-in exclusions (node_modules, .git, etc.)
  --regex                 Treat --f pattern as a regular expression
  --help, -h              Show this help
  --version               Show version

Examples:
  src                                             Show directory tree
  src --r *.rs                                    List all Rust files
  src --r *.ts --f "import"                       Search TypeScript files for imports
  src --f "TODO|FIXME" --pad 2                    Find TODOs with 2 lines of context
  src --f "pub fn" --line-numbers off             Search without line number prefixes
  src --lines "src/main.rs:1:20 src/cli.rs:18:40" Pull exact line ranges
  src --graph                                     Show dependency graph
  src --graph --r *.rs                            Rust-only dependency graph
  src --symbols --r *.rs                          Extract Rust symbol declarations
  src --r *.ts --f "import" --count               Count import occurrences per file
  src --stats                                     Codebase statistics overview
    src -d /path/to/project                         Scan a specific directory
    "#);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(input: &[&str]) -> Vec<String> {
        input.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn no_args_defaults_to_tree_mode() {
        let result = parse_args(&args(&[]));
        match result.unwrap() {
            CliAction::Run(a) => {
                assert!(a.find.is_none());
                assert!(a.globs.is_empty());
                assert!(!a.graph);
                assert!(!a.symbols);
                assert!(!a.stats);
                assert!(!a.count);
                assert!(a.lines.is_empty());
            }
            _ => panic!("Expected Run"),
        }
    }

    #[test]
    fn help_flag() {
        assert!(matches!(parse_args(&args(&["--help"])).unwrap(), CliAction::Help));
        assert!(matches!(parse_args(&args(&["-h"])).unwrap(), CliAction::Help));
        assert!(matches!(parse_args(&args(&["-?"])).unwrap(), CliAction::Help));
    }

    #[test]
    fn version_flag() {
        assert!(matches!(parse_args(&args(&["--version"])).unwrap(), CliAction::Version));
    }

    #[test]
    fn root_directory() {
        match parse_args(&args(&["-d", "/tmp"])).unwrap() {
            CliAction::Run(a) => assert_eq!(a.root, "/tmp"),
            _ => panic!("Expected Run"),
        }
        match parse_args(&args(&["--root", "/tmp"])).unwrap() {
            CliAction::Run(a) => assert_eq!(a.root, "/tmp"),
            _ => panic!("Expected Run"),
        }
    }

    #[test]
    fn glob_patterns() {
        match parse_args(&args(&["--r", "*.rs"])).unwrap() {
            CliAction::Run(a) => {
                assert_eq!(a.globs, vec!["*.rs"]);
            }
            _ => panic!("Expected Run"),
        }
    }

    #[test]
    fn multiple_globs() {
        match parse_args(&args(&["--r", "*.rs", "--r", "*.ts"])).unwrap() {
            CliAction::Run(a) => {
                assert_eq!(a.globs, vec!["*.rs", "*.ts"]);
            }
            _ => panic!("Expected Run"),
        }
    }

    #[test]
    fn search_pattern() {
        match parse_args(&args(&["--f", "pub fn"])).unwrap() {
            CliAction::Run(a) => {
                assert_eq!(a.find, Some("pub fn".to_owned()));
            }
            _ => panic!("Expected Run"),
        }
    }

    #[test]
    fn pad_option() {
        match parse_args(&args(&["--f", "test", "--pad", "3"])).unwrap() {
            CliAction::Run(a) => assert_eq!(a.pad, 3),
            _ => panic!("Expected Run"),
        }
    }

    #[test]
    fn pad_invalid_returns_error() {
        let result = parse_args(&args(&["--pad", "abc"]));
        assert!(result.is_err());
    }

    #[test]
    fn timeout_option() {
        match parse_args(&args(&["--timeout", "10"])).unwrap() {
            CliAction::Run(a) => assert_eq!(a.timeout, Some(10)),
            _ => panic!("Expected Run"),
        }
    }

    #[test]
    fn timeout_invalid_returns_error() {
        let result = parse_args(&args(&["--timeout", "xyz"]));
        assert!(result.is_err());
    }

    #[test]
    fn exclude_option() {
        match parse_args(&args(&["--exclude", "vendor"])).unwrap() {
            CliAction::Run(a) => assert_eq!(a.excludes, vec!["vendor"]),
            _ => panic!("Expected Run"),
        }
    }

    #[test]
    fn no_defaults_flag() {
        match parse_args(&args(&["--no-defaults"])).unwrap() {
            CliAction::Run(a) => assert!(a.no_defaults),
            _ => panic!("Expected Run"),
        }
    }

    #[test]
    fn regex_flag() {
        match parse_args(&args(&["--f", "test", "--regex"])).unwrap() {
            CliAction::Run(a) => assert!(a.is_regex),
            _ => panic!("Expected Run"),
        }
    }

    #[test]
    fn line_numbers_off() {
        match parse_args(&args(&["--line-numbers", "off"])).unwrap() {
            CliAction::Run(a) => assert!(!a.line_numbers),
            _ => panic!("Expected Run"),
        }
    }

    #[test]
    fn line_numbers_invalid_value() {
        let result = parse_args(&args(&["--line-numbers", "yes"]));
        assert!(result.is_err());
    }

    #[test]
    fn lines_option() {
        match parse_args(&args(&["--lines", "src/main.rs:1:20 src/cli.rs:10:30"])).unwrap() {
            CliAction::Run(a) => {
                assert_eq!(a.lines, vec!["src/main.rs:1:20", "src/cli.rs:10:30"]);
            }
            _ => panic!("Expected Run"),
        }
    }

    #[test]
    fn graph_flag() {
        match parse_args(&args(&["--graph"])).unwrap() {
            CliAction::Run(a) => assert!(a.graph),
            _ => panic!("Expected Run"),
        }
    }

    #[test]
    fn symbols_flag() {
        match parse_args(&args(&["--symbols"])).unwrap() {
            CliAction::Run(a) => assert!(a.symbols),
            _ => panic!("Expected Run"),
        }
    }

    #[test]
    fn symbols_short_flag() {
        match parse_args(&args(&["--s"])).unwrap() {
            CliAction::Run(a) => assert!(a.symbols),
            _ => panic!("Expected Run"),
        }
    }

    #[test]
    fn count_requires_find() {
        let result = parse_args(&args(&["--count"]));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("--count requires --f"));
    }

    #[test]
    fn count_with_find_succeeds() {
        match parse_args(&args(&["--f", "test", "--count"])).unwrap() {
            CliAction::Run(a) => {
                assert!(a.count);
                assert_eq!(a.find, Some("test".to_owned()));
            }
            _ => panic!("Expected Run"),
        }
    }

    #[test]
    fn stats_flag() {
        match parse_args(&args(&["--stats"])).unwrap() {
            CliAction::Run(a) => assert!(a.stats),
            _ => panic!("Expected Run"),
        }
    }

    #[test]
    fn stats_short_flag() {
        match parse_args(&args(&["--st"])).unwrap() {
            CliAction::Run(a) => assert!(a.stats),
            _ => panic!("Expected Run"),
        }
    }

    #[test]
    fn mutual_exclusivity_search_and_graph() {
        let result = parse_args(&args(&["--f", "test", "--graph"]));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("mutually exclusive"));
    }

    #[test]
    fn mutual_exclusivity_search_and_symbols() {
        let result = parse_args(&args(&["--f", "test", "--symbols"]));
        assert!(result.is_err());
    }

    #[test]
    fn mutual_exclusivity_graph_and_stats() {
        let result = parse_args(&args(&["--graph", "--stats"]));
        assert!(result.is_err());
    }

    #[test]
    fn mutual_exclusivity_lines_and_graph() {
        let result = parse_args(&args(&["--lines", "f:1:2", "--graph"]));
        assert!(result.is_err());
    }

    #[test]
    fn unknown_option_error() {
        let result = parse_args(&args(&["--unknown"]));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown option"));
    }

    #[test]
    fn missing_value_for_root() {
        let result = parse_args(&args(&["--root"]));
        assert!(result.is_err());
    }

    #[test]
    fn missing_value_for_r() {
        let result = parse_args(&args(&["--r"]));
        assert!(result.is_err());
    }

    #[test]
    fn missing_value_for_f() {
        let result = parse_args(&args(&["--f"]));
        assert!(result.is_err());
    }

    #[test]
    fn missing_value_for_pad() {
        let result = parse_args(&args(&["--pad"]));
        assert!(result.is_err());
    }

    #[test]
    fn missing_value_for_timeout() {
        let result = parse_args(&args(&["--timeout"]));
        assert!(result.is_err());
    }

    #[test]
    fn missing_value_for_exclude() {
        let result = parse_args(&args(&["--exclude"]));
        assert!(result.is_err());
    }

    #[test]
    fn missing_value_for_lines() {
        let result = parse_args(&args(&["--lines"]));
        assert!(result.is_err());
    }

    #[test]
    fn missing_value_for_line_numbers() {
        let result = parse_args(&args(&["--line-numbers"]));
        assert!(result.is_err());
    }

    #[test]
    fn combined_options() {
        match parse_args(&args(&["-d", "/tmp", "--r", "*.rs", "--f", "pub fn", "--pad", "2", "--timeout", "30"])).unwrap() {
            CliAction::Run(a) => {
                assert_eq!(a.root, "/tmp");
                assert_eq!(a.globs, vec!["*.rs"]);
                assert_eq!(a.find, Some("pub fn".to_owned()));
                assert_eq!(a.pad, 2);
                assert_eq!(a.timeout, Some(30));
            }
            _ => panic!("Expected Run"),
        }
    }

    #[test]
    fn default_line_numbers_on() {
        match parse_args(&args(&[])).unwrap() {
            CliAction::Run(a) => assert!(a.line_numbers),
            _ => panic!("Expected Run"),
        }
    }

    #[test]
    fn default_pad_zero() {
        match parse_args(&args(&[])).unwrap() {
            CliAction::Run(a) => assert_eq!(a.pad, 0),
            _ => panic!("Expected Run"),
        }
    }
}
