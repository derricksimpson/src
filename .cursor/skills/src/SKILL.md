---
name: src
description: Fast, parallel source-code scanner that replaces multiple Read/Grep/Glob calls with a single Shell invocation. Use when you need to explore project structure, find files by pattern, search file contents, extract exact line ranges from multiple files at once, show dependency graphs, list symbol declarations, count matches, or gather codebase statistics. Prefer this over sequential Read/Grep/Glob.
---

# src — Fast Parallel Source Code Scanner

`src` is a single-binary CLI that interrogates source code **in parallel**.
One `src` command replaces many sequential `Read`, `Grep`, and `Glob` calls — returning structured YAML (default) or JSON.

**Key principle**: Always prefer a *single* `src` invocation over multiple tool calls. Batch everything you need into one command.

## When to Use `src` (vs built-in tools)

| Scenario | Use `src` | Use built-in tool |
|---|---|---|
| Read 2+ files at once | `src --lines "a.rs:1:50 b.rs:10:30"` | — |
| Find files by name pattern | `src -g "*.ts"` | `Glob` for single simple pattern |
| Search content across many files | `src -f "pattern" -g "*.rs"` | `Grep` only if single file / quick check |
| List functions/classes/structs | `src --symbols -g "*.rs"` | — |
| Understand import structure | `src --graph` | — |
| Get project overview fast | `src` (tree) or `src --stats` | — |
| Fuzzy / semantic search | — | `SemanticSearch` |

## Modes Reference

### 1. Tree (default) — Project structure

```bash
src
src -d /path/to/other/project
```

Returns a nested directory hierarchy of all source files. Use for orientation.

### 2. Glob — Find files by pattern

```bash
src -g "*.rs"
src -g "*.ts" -g "*.tsx"          # multiple globs (repeatable -g)
src -g "*.rs" --limit 5           # cap results
```

Returns a flat list of matching file paths. Combine with `--find` or `--symbols` to filter further.

### 3. Find — Search file contents

```bash
src -f "TODO|FIXME"                          # literal OR search
src -f "handlePayment" -g "*.ts"             # scoped to TypeScript
src -f "async fn.*Result" -g "*.rs" -E       # regex mode
src -f "error" -g "*.go" --no-line-numbers   # cleaner output
```

Returns **full file contents** of every matching file (with line numbers by default).
Add `--count` / `-c` to get match counts per file instead of content.

```bash
src -f "pub fn" -g "*.rs" -c    # counts only — fast triage
```

### 4. Lines — Extract exact ranges from multiple files

```bash
src --lines "src/main.rs:1:20 src/cli.rs:34:50"
src --lines "a.rs:100:120" --lines "b.rs:1:30"   # repeatable
```

Returns precise line ranges from multiple files **in one call**. This is the primary way to batch-read specific code sections instead of multiple `Read` calls.

Output groups lines into `chunks` per file:

```yaml
files:
- path: src/main.rs
  chunks:
  - startLine: 1
    endLine: 20
    content: |
      1.  mod cli;
      2.  mod models;
      ...
- path: src/cli.rs
  chunks:
  - startLine: 34
    endLine: 50
    content: |
      34.  pub fn parse_args(...) {
      ...
```

### 5. Symbols — Extract declarations

```bash
src --symbols -g "*.rs"
src -s -g "*.ts" -g "*.tsx"
src -s --json                    # JSON output
```

Returns function, struct, class, enum, trait, and interface declarations with line ranges.

```yaml
symbols:
- path: src/cli.rs
  funcs:
  - "pub fn parse_args(args: &[String]) -> Result<CliAction, String> {" :34:176
  - pub fn print_help() { :178:236
```

Format: `signature :startLine:endLine`. Use line ranges to then read implementations via `--lines`.

Supported languages: Rust, TypeScript/JavaScript, C#, Go, Java, Kotlin, Ruby, Python.

### 6. Graph — Dependency / import map

```bash
src --graph
src --graph -g "*.rs"            # Rust-only
src --graph -g "*.ts" -g "*.tsx" # TypeScript-only
```

Returns project-internal imports per file:

```yaml
graph:
- file: src/count.rs
  imports:
  - src/file_reader.rs
  - src/models.rs
  - src/path_helper.rs
  - src/searcher.rs
```

### 7. Stats — Codebase overview

```bash
src --stats
src --stats -d /other/project
```

Returns file counts, line counts, and byte sizes grouped by language/extension.

## Common Options

| Flag | Short | Purpose |
|---|---|---|
| `--dir <path>` | `-d` | Set root directory (default: cwd) |
| `--glob <pattern>` | `-g` | File pattern filter (repeatable) |
| `--find <pattern>` | `-f` | Content search pattern (`\|` = OR) |
| `--regex` | `-E` | Treat `--find` as regex |
| `--count` | `-c` | Show match counts (requires `--find`) |
| `--limit <n>` | `-L` | Cap number of files in output |
| `--no-line-numbers` | — | Suppress line-number prefixes |
| `--exclude <name>` | — | Exclude additional dirs/files (repeatable) |
| `--timeout <secs>` | — | Max execution time |
| `--format <fmt>` | `-F` | `yaml` (default) or `json` |
| `--json` | — | Shorthand for `--format json` |
| `--output <path>` | `-o` | Write to file instead of stdout |

## Workflow: Exploring an Unfamiliar Codebase

1. **Orient** — get structure and size:
   ```bash
   src && src --stats
   ```
2. **Find key files** — by name pattern:
   ```bash
   src -g "*controller*" -g "*service*"
   ```
3. **Scan symbols** — discover the API surface:
   ```bash
   src --symbols -g "*.ts"
   ```
4. **Map dependencies** — understand data flow:
   ```bash
   src --graph -g "*.ts"
   ```
5. **Deep-dive** — read specific ranges identified above:
   ```bash
   src --lines "src/auth/controller.ts:1:60 src/auth/service.ts:25:80"
   ```

## Workflow: Investigating a Bug or Feature

1. **Search** for the term across the codebase:
   ```bash
   src -f "handlePayment|processOrder" -g "*.ts" -c
   ```
2. **Read** the matching files (use counts to pick the most relevant):
   ```bash
   src -f "handlePayment" -g "*.ts" --limit 3
   ```
3. **Extract** the exact function and its callers:
   ```bash
   src --symbols -g "*.ts" | grep -i payment   # find line ranges
   src --lines "src/payments.ts:45:90 src/orders.ts:100:140"
   ```

## Workflow: Multi-File Editing Prep

When you need to edit several files, gather all context in one shot:

```bash
src --lines "src/models.rs:1:50 src/cli.rs:34:80 src/main.rs:20:60"
```

This is **dramatically faster** than three separate `Read` calls.

## Important Behavior Notes

- **`--find` returns full file contents** of every matching file (not just matching lines). Use `--count` / `-c` first to triage, then `--limit` to keep output manageable.
- **`--glob` is repeatable** — pass `-g "*.ts" -g "*.tsx"` to match multiple patterns.
- **`--lines` is repeatable** — pass multiple `--lines` flags or space-separate specs in one string.
- **`|` in `--find`** is literal OR (not regex by default). Add `-E` for full regex.
- Built-in exclusions (node_modules, .git, target, dist, etc.) apply by default. Use `--no-defaults` to disable.
- Output is always YAML unless `--json` or `--format json` is specified.
- All modes include a `meta:` block with `elapsedMs` and `filesScanned`/`filesMatched`.

## Anti-Patterns

- **Don't** use multiple sequential `Read` calls when `src --lines` can batch them.
- **Don't** use `Grep` + `Glob` separately when `src -f "pattern" -g "*.ext"` does both.
- **Don't** read entire large files when `--symbols` can tell you which lines matter.
- **Don't** forget `--limit` on broad searches — output can be very large.
