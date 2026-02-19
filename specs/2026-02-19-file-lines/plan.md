# Plan — `--line-numbers off`, `--lines`, and `--graph`

## Current State

`src` is a Rust CLI (9 source files, 3 deps: `rayon`, `memmap2`, `regex`) with three modes:

1. **Directory hierarchy** (default) — tree of folders containing source files
2. **File listing** (`--r <glob>`) — flat list of files matching glob patterns
3. **Content search** (`--f <pattern>`) — parallel grep with context padding, YAML output

All output includes line numbers in the format `1.  line content`. There is no way to suppress them, no way to pull exact line ranges from known files, and no awareness of language-level dependency relationships.

### Current Architecture

```
src/
  main.rs          — entry point, dispatch, signal handling, timeout
  cli.rs           — hand-rolled arg parsing, CliArgs struct, help text
  models.rs        — OutputEnvelope, MetaInfo, FileEntry, FileChunk, ScanResult
  scanner.rs       — parallel dir hierarchy + file listing with globs
  searcher.rs      — parallel content search, mmap, pattern matching, chunk merging
  yaml_output.rs   — hand-rolled YAML emitter (BufWriter, block scalars)
  exclusion.rs     — default + custom directory exclusion filter
  glob.rs          — zero-alloc glob matching (* and ?)
  path_helper.rs   — cross-platform path normalization
```

Line numbers are currently baked into `searcher.rs::build_chunks()` (line 244–265) where each output line is formatted as `{lineNum}.  {lineContent}`. The YAML output layer in `yaml_output.rs` writes these verbatim as block scalars.

---

## Feature 1: `--line-numbers off`

### What it does

Suppresses individual line numbers from file content output. By default, every line in `contents` or `chunks[].content` is prefixed with `{n}.  `. When `--line-numbers off` is provided, content is emitted raw — no line number prefix.

The `startLine` / `endLine` fields on chunks remain present (they describe the range), only the per-line prefix inside the content block is removed.

### Example

```bash
src --r *.rs --f "pub fn" --pad 1 --line-numbers off
```

```yaml
files:
- path: src/scanner.rs
  chunks:
  - startLine: 36
    endLine: 40
    content: |
      pub fn scan_directories(
          root: &Path,
          filter: &ExclusionFilter,
          cancelled: &AtomicBool,
      ) -> ScanResult {
```

vs. current (default):

```yaml
    content: |
      36.  pub fn scan_directories(
      37.      root: &Path,
      38.      filter: &ExclusionFilter,
      39.      cancelled: &AtomicBool,
      40.  ) -> ScanResult {
```

### Implementation

**Files touched:** `cli.rs`, `models.rs` (or pass as param), `searcher.rs`, `main.rs`

1. **`cli.rs`** — Add `line_numbers: bool` to `CliArgs` (default `true`). Parse `--line-numbers off` as a flag. Only valid value is `off`; anything else is an error. Add to help text.

2. **`searcher.rs::build_chunks()`** — Accept a `line_numbers: bool` parameter. When `false`, emit lines without the `{n}.  ` prefix. This is the only place line formatting happens.

3. **`main.rs`** — Thread `args.line_numbers` through `execute_search()` into `searcher::search_files()` and down to `build_chunks()`.

4. **`--lines` mode** (Feature 2 below) also respects this flag.

---

## Feature 2: `--lines`

### What it does

Pulls **specific line ranges** from one or more files. No searching, no globbing — you tell `src` exactly which files and which lines you want, and it returns them. This is the "surgical extraction" mode.

### Syntax

```bash
src --lines "path/to/file.rs:12:22 path/to/other.rs:1:20 src/main.rs:8:18"
```

The value is a **space-separated** list of `<relative-path>:<start-line>:<end-line>` specs. Lines are 1-based, inclusive on both ends.

Multiple `--lines` flags are also accepted and their specs are concatenated:

```bash
src --lines "src/scanner.rs:12:22" --lines "src/searcher.rs:1:20"
```

### Output format

Uses the same YAML `files:` structure with chunks, identical to search output. Each file appears once, with one chunk per requested range (multiple ranges on the same file are merged if overlapping, otherwise appear as separate chunks, sorted by start line).

```bash
src --lines "src/scanner.rs:36:40 src/main.rs:18:21"
```

```yaml
meta:
  elapsedMs: 0
  filesMatched: 2
files:
- path: src/scanner.rs
  chunks:
  - startLine: 36
    endLine: 40
    content: |
      36.  pub fn scan_directories(
      37.      root: &Path,
      38.      filter: &ExclusionFilter,
      39.      cancelled: &AtomicBool,
      40.  ) -> ScanResult {
- path: src/main.rs
  chunks:
  - startLine: 18
    endLine: 21
    content: |
      18.  fn main() {
      19.      let exit_code = run();
      20.      std::process::exit(exit_code);
      21.  }
```

### Error handling

We simply return empty chunks for files that are not found.

| Case | Behavior |
|------|----------|
| File not found | `FileEntry` with `error: "File not found: <path>"` |
| Invalid spec format (missing colons, non-integer) | YAML error: `"Invalid line spec: <spec>"`, exit code 1 |
| Start > end | Swap silently (or reject — swapping is friendlier) |
| Start or end exceeds file length | Clamp to actual file length |
| File is binary | Skip, no entry emitted |
| Permission denied | `FileEntry` with `error` field |

### Interaction with other flags

| Flag | Behavior with `--lines` |
|------|------------------------|
| `--line-numbers off` | Content emitted without per-line number prefix |
| `--root` / `-d` | Specs are resolved relative to root |
| `--timeout` | Enforced as usual |
| `--r`, `--f` | **Mutually exclusive** with `--lines`. Error if combined. |
| `--pad` | Ignored (you asked for exact lines) |

### Implementation

**Files touched:** `cli.rs`, `main.rs`, new `lines.rs` module

1. **`cli.rs`** — Add `lines: Vec<String>` to `CliArgs`. Parse `--lines "<specs>"`, splitting the value on spaces. Accept multiple `--lines` flags. Reject if combined with `--r` or `--f`.

2. **New `lines.rs`** — Contains:
   - `LineSpec { path: String, start: usize, end: usize }` — parsed from `path:start:end`
   - `parse_line_specs(raw: &[String], root: &Path) -> Result<Vec<LineSpec>, String>` — parses and validates all specs
   - `extract_lines(specs: &[LineSpec], root: &Path, line_numbers: bool, cancelled: &AtomicBool) -> Vec<FileEntry>` — reads each file (parallel via rayon), extracts the requested ranges, formats chunks. Groups multiple specs for the same file, merges overlapping ranges, sorts chunks by start line.

3. **`main.rs`** — Add new dispatch branch: if `args.lines` is non-empty, call `lines::extract_lines()`. Wire through `line_numbers`, `timeout`, `cancelled`.

4. **`searcher.rs`** — The `build_chunks()` helper can be reused (or a shared helper extracted) since `--lines` and `--f` both produce chunks with the same format.

---

## Feature 3: `--graph`

### What it does

Inspects source files and emits a **project-internal dependency graph** — which source files depend on which other source files, based on language-specific import/use statements. Only dependencies that resolve to files **within the project** are included; external packages are excluded.

### Syntax

```bash
src --graph
src --graph --r *.rs
src --graph --r "*.ts"
```

When `--r` is provided, the graph is scoped to files matching those globs. Without `--r`, all recognized source files are included.

### Output format

```bash
src --graph
```

```yaml
meta:
  elapsedMs: 3
graph:
- file: src/main.rs
  imports:
  - src/cli.rs
  - src/exclusion.rs
  - src/glob.rs
  - src/models.rs
  - src/path_helper.rs
  - src/scanner.rs
  - src/searcher.rs
  - src/yaml_output.rs
- file: src/scanner.rs
  imports:
  - src/exclusion.rs
  - src/glob.rs
  - src/models.rs
- file: src/searcher.rs
  imports:
  - src/models.rs
  - src/path_helper.rs
- file: src/yaml_output.rs
  imports:
  - src/models.rs
- file: src/cli.rs
  imports: []
- file: src/exclusion.rs
  imports: []
- file: src/glob.rs
  imports: []
- file: src/models.rs
  imports: []
- file: src/path_helper.rs
  imports: []
```

Files with no internal imports are included with `imports: []` for completeness. Files are sorted by path. Only files that exist in the project tree appear in the `imports` list.

### Language-specific parsing

Each language has different import syntax. The graph engine must be **language-pluggable** — a trait/interface that each language module implements. Language modules live in a `src/lang/` subdirectory.

```
src/lang/
  mod.rs       — trait definition, language registry, dispatcher
  rust.rs      — Rust: `mod`, `use crate::`, `use super::`
  csharp.rs    — C#: `using <Namespace>;` mapped to file paths
  typescript.rs — TS/JS: `import ... from './...'`, `require('./...')`
```

#### The trait

```rust
pub trait LangImports {
    /// File extensions this language handler covers.
    fn extensions(&self) -> &[&str];

    /// Given file content and the file's own path, extract relative import
    /// paths that could resolve to other files in the project.
    /// Returns raw module/path references (not yet resolved to actual files).
    fn extract_imports(&self, content: &str, file_path: &Path) -> Vec<String>;
}
```

#### Rust (`lang/rust.rs`)

Patterns to match:
- `mod foo;` — resolves to `{dir}/foo.rs` or `{dir}/foo/mod.rs`
- `use crate::foo::bar` — resolves to `src/foo.rs` or `src/foo/bar.rs` (crate root = `src/`)
- `use super::foo` — resolves relative to parent directory

Only `crate::` and `super::` paths are project-internal. `use std::`, `use rayon::`, etc. are external and ignored.

#### C# (`lang/csharp.rs`)

Patterns to match:
- `using Src.Services;` — map namespace segments to directory path (`Services/`)
- `using Src.Models;` — map to `Models/`

Requires a namespace-to-path convention (common in .NET projects). External namespaces (`System.*`, `Microsoft.*`, NuGet packages) are excluded by checking whether the resolved path exists.

#### TypeScript / JavaScript (`lang/typescript.rs`)

Patterns to match:
- `import { X } from './foo'` — resolves `./foo.ts`, `./foo.tsx`, `./foo/index.ts`
- `import X from '../bar'` — relative path resolution
- `require('./baz')` — CommonJS equivalent
- `export { X } from './foo'` — re-exports

Only relative paths (`./`, `../`) are project-internal. Bare specifiers (`react`, `lodash`) are external and ignored.

### Interaction with other flags

| Flag | Behavior with `--graph` |
|------|------------------------|
| `--r <glob>` | Scope graph to matching files only |
| `--root` / `-d` | Root directory for resolution |
| `--timeout` | Enforced as usual |
| `--exclude`, `--no-defaults` | Applied during file discovery |
| `--f`, `--lines` | **Mutually exclusive** with `--graph`. Error if combined. |
| `--line-numbers` | No effect (graph has no content blocks) |
| `--pad` | No effect |

### Implementation

**Files touched:** `cli.rs`, `main.rs`, `models.rs`, `yaml_output.rs`, new `src/lang/` directory

1. **`cli.rs`** — Add `graph: bool` to `CliArgs`. Parse `--graph` flag. Reject if combined with `--f` or `--lines`. Add to help text.

2. **`models.rs`** — Add:
   ```rust
   pub struct GraphEntry {
       pub file: String,
       pub imports: Vec<String>,
   }
   ```
   Add `graph: Option<Vec<GraphEntry>>` to `OutputEnvelope`.

3. **New `src/lang/mod.rs`** — Contains:
   - `LangImports` trait (as above)
   - `fn get_handler(extension: &str) -> Option<&dyn LangImports>` — returns the right handler for a file extension
   - Registration of all language handlers

4. **New `src/lang/rust.rs`** — `RustImports` implementing `LangImports`. Scans for `mod` declarations and `use crate::`/`use super::` statements. Resolves to file paths relative to crate root.

5. **New `src/lang/csharp.rs`** — `CSharpImports` implementing `LangImports`. Scans for `using` directives, maps namespace to path, filters to project-internal only.

6. **New `src/lang/typescript.rs`** — `TypeScriptImports` implementing `LangImports`. Scans for `import`/`require`/`export` with relative paths, resolves with extension probing (`.ts`, `.tsx`, `.js`, `.jsx`, `/index.*`).

7. **New `graph.rs`** — Orchestrator:
   - Takes list of files (from `scanner::find_files` or all source files)
   - For each file: detect language from extension, call `lang::get_handler()`, extract imports, resolve to actual files in the project
   - Build `Vec<GraphEntry>` sorted by file path
   - Parallel via rayon

8. **`yaml_output.rs`** — Add `write_graph()` to emit the `graph:` section. Simple list of mappings with `file:` and `imports:` (flow sequence `[]` when empty, block sequence otherwise).

9. **`main.rs`** — Add dispatch: if `args.graph`, run file discovery then `graph::build_graph()`. Wire through `timeout`, `cancelled`.

### Adding new languages

To add a new language (e.g., Go, Python, Java):

1. Create `src/lang/go.rs` implementing `LangImports`
2. Register it in `src/lang/mod.rs`
3. Done. No other files change.

---

## Dispatch Matrix (updated)

| Condition | Mode |
|-----------|------|
| No `--r`, no `--f`, no `--lines`, no `--graph` | Directory hierarchy |
| `--r` only | File listing |
| `--f` present (with or without `--r`) | Content search |
| `--lines` present | Line extraction |
| `--graph` present (with or without `--r`) | Dependency graph |

**Mutual exclusions:** `--f`, `--lines`, and `--graph` cannot be combined with each other.

---

## Task Breakdown

- [ ] 1. Add `--line-numbers off` support
  - [ ] 1.1 Add `line_numbers: bool` to `CliArgs`, parse `--line-numbers off`
  - [ ] 1.2 Thread through `main.rs` → `searcher.rs`
  - [ ] 1.3 Modify `build_chunks()` to conditionally omit line prefixes
  - [ ] 1.4 Update help text

- [ ] 2. Add `--lines` mode
  - [ ] 2.1 Add `lines: Vec<String>` to `CliArgs`, parse `--lines`
  - [ ] 2.2 Add mutual exclusion validation (`--lines` vs `--f`/`--graph`)
  - [ ] 2.3 Create `lines.rs` — `LineSpec`, `parse_line_specs()`, `extract_lines()`
  - [ ] 2.4 Wire into `main.rs` dispatch
  - [ ] 2.5 Update help text with examples

- [ ] 3. Add `--graph` mode
  - [ ] 3.1 Add `graph: bool` to `CliArgs`, parse `--graph`
  - [ ] 3.2 Add `GraphEntry` to `models.rs`, extend `OutputEnvelope`
  - [ ] 3.3 Create `src/lang/mod.rs` — `LangImports` trait, registry
  - [ ] 3.4 Create `src/lang/rust.rs` — `mod`, `use crate::`, `use super::`
  - [ ] 3.5 Create `src/lang/typescript.rs` — `import`/`require` with relative paths
  - [ ] 3.6 Create `src/lang/csharp.rs` — `using` namespace-to-path mapping
  - [ ] 3.7 Create `graph.rs` — orchestrator: file discovery → extract → resolve → output
  - [ ] 3.8 Add `write_graph()` to `yaml_output.rs`
  - [ ] 3.9 Wire into `main.rs` dispatch
  - [ ] 3.10 Update help text with examples

---

## Updated CLI Help (target state)

```
src — fast source code interrogation tool

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
```
