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
}

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
            "--no-defaults" => no_defaults = true,
            "--regex" => is_regex = true,
            "--help" | "-h" | "-?" => return Ok(CliAction::Help),
            "--version" => return Ok(CliAction::Version),
            other => return Err(format!("Unknown option: {}\nRun 'src --help' for usage information.", other)),
        }
        i += 1;
    }

    let mut exclusive_count = 0;
    let mut exclusive_names = Vec::new();
    if find.is_some() { exclusive_count += 1; exclusive_names.push("--f"); }
    if !lines.is_empty() { exclusive_count += 1; exclusive_names.push("--lines"); }
    if graph { exclusive_count += 1; exclusive_names.push("--graph"); }
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

Options:
  --root, -d <path>       Root directory (default: current directory)
  --r <glob>              File glob pattern (repeatable, e.g. --r *.ts --r *.cs)
  --f <pattern>           Search pattern (use | for OR, e.g. Payment|Invoice)
  --lines "<specs>"       Line specs: "file:start:end file2:start:end" (repeatable)
  --graph                 Emit source dependency graph
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
  src -d /path/to/project                         Scan a specific directory
"#);
}
