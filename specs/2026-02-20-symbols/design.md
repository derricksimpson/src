# Design Document — `--symbols`, `--count`, `--stats`

## Overview

This design adds three new modes to `src`: symbol extraction (`--symbols`/`--s`), match counting (`--count`), and codebase statistics (`--stats`/`--st`). Each follows the established pattern — discover files via `scanner::find_files`, process them in parallel with rayon, collect results into models, and emit YAML through the hand-rolled writer.

The key architectural addition is a new `LangSymbols` trait in the `lang/` module that mirrors the existing `LangImports` pattern. Each language handler gains a second trait implementation for symbol extraction. The `--count` and `--stats` modes are simpler — they reuse existing infrastructure with new output shapes.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      CLI Layer (cli.rs)                       │
│  New flags: --symbols/--s, --count, --stats/--st             │
│  Updated mutual exclusion: --f, --lines, --graph,            │
│    --symbols, --stats, --f+--count all mutually exclusive    │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                   Dispatch Layer (main.rs)                    │
│  New branches:                                               │
│    args.symbols  → execute_symbols()                         │
│    args.stats    → execute_stats()                           │
│    args.count    → execute_count()                           │
└──────────────────────────┬──────────────────────────────────┘
                           │
              ┌────────────┼────────────┐
              ▼            ▼            ▼
┌──────────────────┐ ┌──────────┐ ┌──────────────┐
│  symbols.rs      │ │ count.rs │ │  stats.rs    │
│  (new module)    │ │ (new)    │ │  (new)       │
│                  │ │          │ │              │
│  Uses lang/      │ │ Uses     │ │  Uses        │
│  LangSymbols     │ │ searcher │ │  scanner     │
│  trait           │ │ Matcher  │ │  find_files  │
└──────────────────┘ └──────────┘ └──────────────┘
              │
              ▼
┌─────────────────────────────────────────────────────────────┐
│                   lang/ Module (extended)                     │
│  Existing: LangImports trait + handlers                      │
│  New: LangSymbols trait + handlers                           │
│                                                              │
│  lang/mod.rs    — new trait, new registry, get_symbol_handler│
│  lang/rust.rs   — impl LangSymbols for RustImports           │
│  lang/typescript.rs — impl LangSymbols for TypeScriptImports │
│  lang/csharp.rs — impl LangSymbols for CSharpImports         │
└─────────────────────────────────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────────────────────┐
│               Models + YAML Output (extended)                │
│  models.rs      — SymbolEntry, SymbolFile, CountEntry,       │
│                   StatsLanguage, StatsLargest, StatsResult    │
│  yaml_output.rs — write_symbols(), write_counts(),           │
│                   write_stats()                              │
│  OutputEnvelope — new optional fields: symbols, counts, stats│
└─────────────────────────────────────────────────────────────┘
```

## Components and Interfaces

### 1. `LangSymbols` Trait (`lang/mod.rs`)

A new trait alongside `LangImports`. Kept separate so language handlers can implement one, both, or neither.

```rust
pub struct SymbolInfo {
    pub kind: &'static str,     // "fn", "struct", "class", "enum", "trait", "interface", "type", "const", "method", "mod"
    pub name: String,
    pub line: usize,            // 1-based
    pub visibility: Option<&'static str>,  // "pub", "export", "public", "private", "protected", None
    pub parent: Option<String>, // containing type name for methods
    pub signature: String,      // the declaration line, trimmed
}

pub trait LangSymbols: Sync {
    fn extensions(&self) -> &[&str];
    fn extract_symbols(&self, content: &str) -> Vec<SymbolInfo>;
}
```

Registration follows the same pattern as `LangImports`:

```rust
static SYMBOL_HANDLERS: &[&dyn LangSymbols] = &[
    &rust::RustImports,
    &typescript::TypeScriptImports,
    &csharp::CSharpImports,
];

pub fn get_symbol_handler(extension: &str) -> Option<&'static dyn LangSymbols> { ... }
```

Each existing handler struct (`RustImports`, `TypeScriptImports`, `CSharpImports`) gains an `impl LangSymbols` block. The struct names stay the same — they're just capability containers.

### 2. Rust Symbol Extraction (`lang/rust.rs`)

Scans line-by-line. Tracks current `impl` block context (the type name) to assign `parent` for methods.

Patterns:
- `pub fn name(` / `fn name(` → kind=fn (or kind=method if inside impl)
- `pub struct Name` / `struct Name` → kind=struct
- `pub enum Name` / `enum Name` → kind=enum
- `pub trait Name` / `trait Name` → kind=trait
- `pub type Name` / `type Name` → kind=type
- `pub const NAME` / `const NAME` → kind=const
- `impl TypeName` → sets parent context (not emitted as a symbol itself)
- `mod name;` → kind=mod

Visibility detection: if the line starts with `pub ` or `pub(crate) ` → visibility = "pub".

Impl tracking: when we see `impl SomeType` (with or without generics), set `current_parent = "SomeType"`. Reset when we encounter a line that's a closing `}` at indent level 0.

Signature: the full trimmed line, up to the opening `{` or end of line.

### 3. TypeScript/JavaScript Symbol Extraction (`lang/typescript.rs`)

Patterns:
- `export function name(` / `function name(` → kind=fn
- `export default function name(` → kind=fn, visibility=export
- `export class Name` / `class Name` → kind=class
- `export interface Name` / `interface Name` → kind=interface
- `export type Name` / `type Name` → kind=type
- `export enum Name` / `enum Name` → kind=enum
- `export const NAME` / `const NAME =` (top-level) → kind=const
- Arrow function: `export const name = (` or `const name = (` → kind=fn
- `export default` → kind=export (only if standalone)

Visibility: `export` prefix → visibility = "export". No prefix → None.

Class method tracking: when inside a class body (tracked via brace depth after a `class` declaration), method declarations (lines matching `name(` or `async name(` or visibility keywords like `private`/`public`/`protected`) get `parent` set to the class name.

### 4. C# Symbol Extraction (`lang/csharp.rs`)

Patterns:
- `public class Name` / `internal class Name` / `class Name` → kind=class
- `public interface Name` / `interface Name` → kind=interface
- `public struct Name` / `struct Name` → kind=struct
- `public enum Name` / `enum Name` → kind=enum
- `namespace Name` → kind=namespace
- Method declarations inside classes: `public ReturnType Name(` → kind=method, parent=class name
- `public const` / `const` → kind=const

Visibility: `public`, `private`, `protected`, `internal`, `protected internal`.

### 5. `symbols.rs` — Symbol Extraction Orchestrator (new module)

```rust
pub fn extract_symbols(
    file_paths: &[String],
    root: &Path,
    cancelled: &AtomicBool,
) -> Vec<SymbolFile>
```

Flow:
1. Receive file paths (from `scanner::find_files`)
2. For each file (parallel via rayon):
   a. Detect language from extension via `lang::get_symbol_handler`
   b. If no handler, skip
   c. Read file content (buffered for small, mmap for large — reuse the same thresholds and binary detection from `searcher.rs`)
   d. Call `handler.extract_symbols(&content)`
   e. Build `SymbolFile { path, symbols }`
3. Sort results by path
4. Return

### 6. `count.rs` — Match Counting (new module)

```rust
pub struct CountResult {
    pub path: String,
    pub count: usize,
}

pub fn count_matches(
    file_paths: &[String],
    root: &Path,
    matcher: &Matcher,
    cancelled: &AtomicBool,
) -> (Vec<CountResult>, usize)  // (per-file counts, total)
```

Flow:
1. Reuse `searcher::Matcher` for pattern matching
2. For each file (parallel via rayon):
   a. Read file (same mmap/buffered strategy)
   b. Count lines where `matcher.is_match(line)` is true
   c. If count > 0, include in results
3. Sort by path
4. Sum total across all files
5. Return

This is essentially the search pipeline with the chunk-building stripped out. We can either refactor `searcher.rs` to share the file-reading logic, or duplicate the small read helpers in `count.rs`. Given the simplicity (< 50 lines of file-reading code), a light duplication is fine to avoid coupling.

### 7. `stats.rs` — Codebase Statistics (new module)

```rust
pub struct LangStats {
    pub extension: String,
    pub files: usize,
    pub lines: usize,
    pub bytes: u64,
}

pub struct LargestFile {
    pub path: String,
    pub lines: usize,
    pub bytes: u64,
}

pub struct StatsResult {
    pub languages: Vec<LangStats>,
    pub totals_files: usize,
    pub totals_lines: usize,
    pub totals_bytes: u64,
    pub largest: Vec<LargestFile>,
}

pub fn compute_stats(
    file_paths: &[String],
    root: &Path,
    cancelled: &AtomicBool,
) -> StatsResult
```

Flow:
1. For each file (parallel via rayon):
   a. Get file metadata (size in bytes)
   b. Count lines (read file, count newlines — or use mmap for large files)
   c. Extract extension
   d. Collect `(extension, lines, bytes, path)`
2. Aggregate by extension → `Vec<LangStats>`, sorted by lines descending
3. Compute totals
4. Sort all files by bytes descending, take top 10 → `Vec<LargestFile>`
5. Return

For line counting, we don't need to split into `Vec<&str>` — just count `\n` bytes in the raw data. For mmap'd files this is a single pass over the memory-mapped region. Extremely fast.

### 8. Updated `CliArgs` (`cli.rs`)

New fields:
```rust
pub struct CliArgs {
    // ... existing fields ...
    pub symbols: bool,    // --symbols or --s
    pub count: bool,      // --count
    pub stats: bool,      // --stats or --st
}
```

Updated mutual exclusion check — the exclusive set becomes: `--f` (alone), `--lines`, `--graph`, `--symbols`, `--stats`. Additionally, `--count` requires `--f` and makes `--f+--count` exclusive with the others.

### 9. Updated `OutputEnvelope` (`models.rs`)

```rust
pub struct OutputEnvelope {
    pub meta: Option<MetaInfo>,
    pub files: Option<Vec<FileEntry>>,
    pub tree: Option<ScanResult>,
    pub graph: Option<Vec<GraphEntry>>,
    pub symbols: Option<Vec<SymbolFile>>,
    pub counts: Option<Vec<CountEntry>>,
    pub stats: Option<StatsOutput>,
    pub error: Option<String>,
}
```

New models:

```rust
pub struct SymbolEntry {
    pub kind: String,
    pub name: String,
    pub line: usize,
    pub visibility: Option<String>,
    pub parent: Option<String>,
    pub signature: String,
}

pub struct SymbolFile {
    pub path: String,
    pub symbols: Vec<SymbolEntry>,
    pub error: Option<String>,
}

pub struct CountEntry {
    pub path: String,
    pub count: usize,
}

pub struct StatsOutput {
    pub languages: Vec<LangStats>,
    pub totals: StatsTotals,
    pub largest: Vec<LargestFile>,
}

pub struct StatsTotals {
    pub files: usize,
    pub lines: usize,
    pub bytes: u64,
}
```

`MetaInfo` gains an optional `total_matches: Option<usize>` for `--count` mode.

### 10. Updated YAML Output (`yaml_output.rs`)

Three new write functions following the existing pattern:

**`write_symbols`**: Emits:
```yaml
files:
- path: src/searcher.rs
  symbols:
  - kind: enum
    name: Matcher
    line: 16
    visibility: pub
    signature: "pub enum Matcher {"
  - kind: fn
    name: search_files
    line: 74
    visibility: pub
    signature: "pub fn search_files(file_paths: &[String], root: &Path, ...) -> Vec<FileEntry>"
```

**`write_counts`**: Emits:
```yaml
files:
- path: src/searcher.rs
  count: 8
- path: src/lines.rs
  count: 6
```

**`write_stats`**: Emits:
```yaml
languages:
- extension: rs
  files: 15
  lines: 1247
  bytes: 38420
- extension: md
  files: 8
  lines: 1735
  bytes: 77800
totals:
  files: 23
  lines: 2982
  bytes: 116220
largest:
- path: src/searcher.rs
  lines: 280
  bytes: 8400
```

### 11. Updated Dispatch (`main.rs`)

New dispatch branches, ordered by priority:

```rust
if !args.lines.is_empty() {
    execute_lines(...)
} else if args.graph {
    execute_graph(...)
} else if args.symbols {
    execute_symbols(...)
} else if args.stats {
    execute_stats(...)
} else if args.count && args.find.is_some() {
    execute_count(...)
} else if let Some(ref find_pattern) = args.find {
    execute_search(...)
} else if !args.globs.is_empty() {
    execute_file_listing(...)
} else {
    execute_directory_hierarchy(...)
}
```

## Data Models

### Symbol Output Example

```yaml
meta:
  elapsedMs: 3
  filesScanned: 15
  filesMatched: 12
files:
- path: src/cli.rs
  symbols:
  - kind: struct
    name: CliArgs
    line: 1
    visibility: pub
    signature: "pub struct CliArgs {"
  - kind: enum
    name: CliAction
    line: 15
    visibility: pub
    signature: "pub enum CliAction {"
  - kind: fn
    name: parse_args
    line: 21
    visibility: pub
    signature: "pub fn parse_args(args: &[String]) -> Result<CliAction, String> {"
  - kind: fn
    name: print_help
    line: 122
    visibility: pub
    signature: "pub fn print_help() {"
```

### Count Output Example

```yaml
meta:
  elapsedMs: 2
  filesScanned: 15
  filesMatched: 8
  totalMatches: 23
files:
- path: src/searcher.rs
  count: 8
- path: src/lines.rs
  count: 6
- path: src/graph.rs
  count: 5
- path: src/main.rs
  count: 4
```

### Stats Output Example

```yaml
meta:
  elapsedMs: 5
  filesScanned: 23
  filesMatched: 23
languages:
- extension: md
  files: 8
  lines: 1735
  bytes: 77800
- extension: rs
  files: 15
  lines: 1247
  bytes: 38420
totals:
  files: 23
  lines: 2982
  bytes: 116220
largest:
- path: specs/2026-02-18-initial-CLI/design.md
  lines: 348
  bytes: 15200
- path: src/searcher.rs
  lines: 280
  bytes: 8400
- path: src/yaml_output.rs
  lines: 170
  bytes: 5100
```

## Error Handling

| Scenario | Behavior |
|----------|----------|
| `--symbols` with unsupported file extension | Skip file, no entry emitted |
| `--symbols` with unreadable file | `SymbolFile` with `error` field, continue |
| `--symbols` with binary file | Skip, no entry |
| `--count` without `--f` | Error: `"--count requires --f <pattern>"`, exit 1 |
| `--count` with invalid regex | Error: `"Invalid regex: ..."`, exit 1 |
| `--stats` with unreadable file | Skip, not counted |
| `--stats` with binary file | Include in byte count, lines = 0 |
| Mutual exclusion violation | Error listing conflicting flags, exit 1 |
| Timeout during any new mode | Partial results, `meta.timeout: true`, exit 2 |
| Ctrl+C during any new mode | Same cancellation path as existing modes |
