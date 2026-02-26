# src

A fast, single-binary CLI for interrogating source code. One command can replace dozens of `grep`, `find`, and `cat` calls with structured YAML or JSON output — scanning files in parallel.

It is built in Rust and is optimized for speed and memory usage (with even more planned optimizations coming soon).

Built for AI agents and power users who need to understand codebases quickly.

## Install

```bash
cargo install --path .
```

Or build from source:

```bash
cargo build --release
# binary at target/release/src
```

## Quick Start

```bash
src                                          # project tree
src -g "*.rs"                                # find files by pattern
src -f "TODO|FIXME"                          # search contents (full files returned)
src -f "handlePayment" -g "*.ts" -c          # count matches per file
src --lines "src/main.rs:1:20 lib.rs:10:40"  # extract exact line ranges
src --symbols -g "*.rs"                      # list function/struct/class declarations
src --graph -g "*.ts"                        # show internal dependency graph
src --stats                                  # codebase statistics by language
```

## Modes

| Mode | Flag | Description |
|------|------|-------------|
| **Tree** | *(default)* | Directory hierarchy of source files |
| **Glob** | `-g <pattern>` | List files matching glob patterns (repeatable) |
| **Find** | `-f <pattern>` | Search file contents (`\|` for OR, `-E` for regex) |
| **Count** | `-f <pattern> -c` | Match counts per file |
| **Lines** | `--lines "<specs>"` | Extract specific line ranges from multiple files |
| **Symbols** | `-s` | Extract declarations (functions, classes, structs, etc.) |
| **Graph** | `--graph` | Project-internal import/dependency map |
| **Stats** | `-S` | File counts, line counts, bytes by language |

## Supported Languages

Import resolution (`--graph`) and symbol extraction (`--symbols`) work for:

- **Rust** — `use`/`mod`, fn/struct/enum/trait/impl
- **TypeScript / JavaScript** — `import`/`require`, function/class/interface/type/const
- **C#** — `using`, class/struct/interface/enum/method
- **Go** — `import`, func/type/method/const/var
- **Java** — `import`, class/interface/enum/record/method
- **Kotlin** — `import`, class/data class/object/fun/val
- **Ruby** — `require`/`require_relative`, class/module/def
- **Python** — `import`/`from...import`, class/def

All other file types are still included in tree, glob, find, lines, and stats modes.

## Options

```
--dir, -d <path>        Root directory (default: cwd)
--glob, -g <pattern>    File glob filter (repeatable)
--find, -f <pattern>    Content search (| for OR)
--regex, -E             Treat --find as regex
--count, -c             Show match counts (requires --find)
--lines "<specs>"       Line ranges: "file:start:end" (repeatable)
--symbols, -s           Extract symbol declarations
--graph                 Dependency graph
--stats, -S             Codebase statistics
--limit, -L <n>         Max files in output
--no-line-numbers       Suppress line number prefixes
--exclude <name>        Additional exclusion (repeatable)
--no-defaults           Disable built-in exclusions
--timeout <secs>        Max execution time
--format, -F <fmt>      yaml (default) or json
--json                  Shorthand for --format json
--output, -o <path>     Write to file instead of stdout
```

## Output Format

Default output is YAML-like structured text (optimized for LLM consumption). Use `--json` for machine-readable JSON.

Every response includes a `meta` block:

```yaml
meta:
  elapsedMs: 16
  filesScanned: 60
  filesMatched: 28
```

Errors are collected non-fatally:

```yaml
meta:
  filesErrored: 1
errors:
- "permission denied: secret.key"
```

## Examples

### Search and extract

```bash
# Find all payment-related code in TypeScript
src -f "payment|invoice" -g "*.ts" -c

# Read the top 3 matching files
src -f "payment" -g "*.ts" --limit 3

# Extract specific functions by line range
src --lines "src/payments.ts:45:90 src/orders.ts:100:140"
```

### Understand structure

```bash
# What does this project look like?
src && src --stats

# What functions exist in the auth module?
src -s -g "*auth*"

# How do files depend on each other?
src --graph -g "*.rs"
```

### Batch reads

```bash
# Read 5 files in one shot instead of 5 separate commands
src --lines "a.rs:1:50 b.rs:10:30 c.ts:1:100 d.go:20:40 e.py:1:25"
```

## Built-in Exclusions

By default, these directories are excluded: `node_modules`, `.git`, `target`, `dist`, `build`, `bin`, `obj`, `.next`, `__pycache__`, `vendor`, and others. Use `--no-defaults` to disable, or `--exclude <name>` to add more.

## Architecture

~12K lines of Rust. Zero runtime dependencies beyond the standard library and three crates:

- [`rayon`](https://crates.io/crates/rayon) — parallel file scanning
- [`memmap2`](https://crates.io/crates/memmap2) — memory-mapped file I/O
- [`regex`](https://crates.io/crates/regex) — regex search mode

Release binary is ~1.5 MB with LTO and strip enabled.

## Tests

662 tests (588 unit + 74 integration):

```bash
cargo test
```

## License

MIT
