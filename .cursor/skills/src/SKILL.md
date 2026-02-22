---
name: src
description: Fast source code scanner. Use when you need to explore a codebase, find files, search contents, extract line ranges, show dependency graphs, extract symbol declarations, count matches, or get codebase statistics. All in parallel with a single execution!
---

# src — Fast Source Code Scanner

`src` is a single-binary CLI for interrogating source code. It replaces multiple `Read`/`Grep`/`Glob` calls with one command - capable of reading multiple files in one command and in parallel. Output is structured YAML (default) or JSON.

## Modes
src — extremely fast source code interrogation tool for retreiving  details from many source code in parallel and one execution.

Usage:
  src [options]

Modes:
  (default)               Show directory hierarchy containing source files
  --glob, -g <glob>       List files matching glob patterns (repeatable)
  --find, -f <pattern>    Search file contents for a pattern
  --lines "<specs>"       Extract specific line ranges from files
  --graph                 Show project-internal dependency graph
  --symbols, -s           Extract symbol declarations from source files
  --stats, -S             Show codebase statistics (files, lines, bytes by language)

Options:
  --dir, -d <path>        Root directory (default: current directory)
  --glob, -g <glob>       File glob pattern (repeatable, e.g. -g *.ts -g *.cs)
  --find, -f <pattern>    Search pattern (use | for OR, e.g. Payment|Invoice)
  --lines "<specs>"       Line specs: "file:start:end file2:start:end" (repeatable)
  --graph                 Emit source dependency graph
  --symbols, -s           Extract symbol declarations (compact: signature :start:end)
  --count, -c             Show match counts per file (requires --find)
  --stats, -S             File counts, line counts, byte sizes by extension
  --limit, -L <n>         Max number of files in the output
  --no-line-numbers       Suppress per-line number prefixes in content output
  --timeout <secs>        Max execution time in seconds
  --exclude <name>        Additional exclusions (repeatable)
  --no-defaults           Disable built-in exclusions (node_modules, .git, etc.)
  --regex, -E             Treat --find pattern as a regular expression
  --format, -F <fmt>      Output format: yaml (default) or json
  --json                  Shorthand for --format json
  --output, -o <path>     Write output to file instead of stdout
  --help, -h              Show this help
  --version, -V           Show version

Aliases:
  --r, --f, --s, --st, --root, --line-numbers off
  Legacy aliases are kept for backward compatibility.

Examples:
  src                                             Show directory tree
  src -g *.rs                                     List all Rust files
  src -g *.ts -f "import"                         Search TypeScript files for imports
  src -f "TODO|FIXME"                             Find TODOs (full file content returned)
  src -f "pub fn" --no-line-numbers               Search without line number prefixes
  src --lines "src/main.rs:1:20 src/cli.rs:18:40" Pull exact line ranges
  src --graph                                     Show dependency graph
  src --graph -g *.rs                             Rust-only dependency graph
  src -s -g *.rs                                  Extract Rust symbol declarations
  src -g *.ts -f "import" -c                      Count import occurrences per file
  src --stats                                     Codebase statistics overview
  src -d /path/to/project                         Scan a specific directory
  src -f "TODO" --limit 10                        Find TODOs, cap at 10 files
  src --symbols --json                            Symbols in JSON format
  src --stats -o stats.yaml                       Write stats to file

