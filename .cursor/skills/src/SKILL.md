---
name: src
description: Fast source code scanner. Use when you need to explore a codebase, find files, search contents, extract line ranges, show dependency graphs, extract symbol declarations, count matches, or get codebase statistics. All in parallel!
---

# src — Fast Source Code Scanner

`src` is a single-binary CLI for interrogating source code. It replaces multiple `Read`/`Grep`/`Glob` calls with one command - capable of reading multiple files in one command and in parallel. Output is always structured YAML.

## Modes

| Mode | Flag | Description |
|------|------|-------------|
| Tree | *(default)* | Directory hierarchy of source files |
| File list | `--r <glob>` | List files matching globs (repeatable) |
| Search | `--f <pattern>` | Search file contents (`\|` for OR, `--regex` for regex) |
| Lines | `--lines "<specs>"` | Extract exact line ranges (repeatable) |
| Graph | `--graph` | Project-internal dependency graph |
| Symbols | `--symbols` / `--s` | Extract fn/struct/class/enum/trait declarations |
| Count | `--f <pat> --count` | Per-file match counts (requires `--f`) |
| Stats | `--stats` / `--st` | Codebase statistics by language/extension |

Modes are mutually exclusive: `--f`, `--lines`, `--graph`, `--symbols`, `--stats`, and `--f --count` cannot be combined with each other.

## Options

| Option | Purpose |
|--------|---------|
| `--r <glob>` | Filter by file glob (repeatable, scopes any mode) |
| `--pad <n>` | Context lines around matches (search mode) |
| `--line-numbers off` | Suppress line number prefixes in content |
| `--regex` | Treat `--f` pattern as regex |
| `-d <path>` | Set root directory |
| `--exclude <name>` | Add exclusion (repeatable) |
| `--no-defaults` | Disable built-in exclusions |
| `--timeout <secs>` | Execution time limit |

## Quick Reference

```bash
# Tree
src
src -d /path/to/project

# File listing
src --r "*.rs"
src --r "*.ts" --r "*.tsx"

# Search
src --r "*.rs" --f "pub fn|pub struct"
src --f "TODO|FIXME" --pad 2

# Read full files (search trick: match everything with high pad)
src --r "*.rs" --f "." --pad 999
src --r "*.rs" --f "." --pad 999 --line-numbers off

# Extract exact line ranges
src --lines "src/main.rs:1:20 src/cli.rs:18:40"
src --lines "src/main.rs:1:10" --lines "src/cli.rs:1:5"

# Dependency graph
src --graph
src --graph --r "*.rs"

# Symbol extraction
src --symbols
src --symbols --r "*.rs"
src --s --r "*.ts"

# Match counting
src --r "*.rs" --f "pub fn" --count
src --f "import" --count

# Codebase statistics
src --stats
src --stats --r "*.rs"
src --st
```

## Output Shapes

**Tree** — `tree.children[].name`, `tree.files[]`
**File list** — `files[].path`
**Search / Lines** — `files[].contents` or `files[].chunks[].content` with `startLine`/`endLine`
**Graph** — `graph[].file`, `graph[].imports[]`
**Symbols** — `files[].path`, `files[].symbols[].kind/name/line/visibility/parent/signature`
**Count** — `files[].path`, `files[].count`, `meta.totalMatches`
**Stats** — `languages[].extension/files/lines/bytes`, `totals`, `largest[].path/lines/bytes`

All modes include `meta.elapsedMs` and `meta.filesMatched`.

## Supported Languages

**Graph** (`--graph`) — Rust, TypeScript/JS, C#, Go, Python
**Symbols** (`--symbols`) — Rust, TypeScript/JS, C#, Go, Python

Symbol kinds: `fn`, `method`, `struct`, `class`, `enum`, `trait`, `interface`, `type`, `const`, `mod`, `namespace`, `var`, `export`

## When to Use src vs Other Tools

| Scenario | Use src | Use other tools |
|----------|---------|-----------------|
| Context from many files | `--r` + `--f` + `--pad` | Too many Read calls |
| Exploring unfamiliar codebase | tree, then search | No |
| Code structure overview | `--symbols` or `--stats` | No equivalent |
| Pattern frequency analysis | `--f` + `--count` | No equivalent |
| Module dependencies | `--graph` | No equivalent |
| Know exact file + lines | `--lines` | Read with offset works too |
| Single known file | No | Read tool is simpler |
| Exact text in 1-2 files | No | Grep tool is simpler |

## Tips

- **Batch context gathering**: one `src --r "*.ts" --f "import" --pad 3` replaces dozens of reads.
- **Start broad, narrow down**: tree → file list → symbols/search → `--lines` for surgical reads.
- **Use `--symbols` to understand architecture**: shows every declaration without reading full files.
- **Use `--count` for hotspot analysis**: find where a pattern appears most.
- **Use `--stats` for project profiling**: instant breakdown by language, top files by size.
- **Use `--graph` before refactoring**: understand the blast radius of changes.
- **`--line-numbers off`** is ideal when feeding output to LLMs.
- **`|` in `--f`** is lightweight OR — no `--regex` needed for multi-term search.
