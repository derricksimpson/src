---
name: src
description: Rapidly scan and gather source code context using the src CLI tool. Use when you need to explore a codebase, find files by pattern, search file contents, read multiple files at once, extract exact line ranges, or understand dependency structure. Triggers on requests like "scan the code", "find all files matching", "search for", "show me the codebase structure", "read lines 10-50", "show the dependency graph", or when you need broad codebase context.
---

# src — Fast Source Code Scanner

The `src` CLI (`src` or `src.exe` on Windows) is a blazing-fast tool for interrogating source code. Use it instead of multiple `Read`/`Grep`/`Glob` calls when you need context from many files at once.

## Finding the Binary

Look for `src` (Linux/macOS) or `src.exe` (Windows) relative to the workspace root. If not found, check if a `Cargo.toml` with `name = "src-cli"` exists and build with `cargo build --release`.

## Five Modes

### 1. Directory Tree (default, no flags)

```bash
src
src -d /path/to/project
```

Returns YAML tree of the project structure. Use this first to orient yourself in an unfamiliar codebase.

### 2. File Listing (`--r <glob>`)

```bash
src --r "*.rs"
src --r "*.ts" --r "*.tsx"
```

Lists files matching glob patterns. Repeatable — use multiple `--r` flags for multiple extensions. Use this to discover which files exist before reading them.

### 3. Content Search (`--f <pattern>`)

```bash
src --r "*.rs" --f "pub fn|pub struct"
src --f "TODO|FIXME" --pad 2
src --r "*.ts" --f "export default" --pad 5
```

Searches file contents. Combine with `--r` to scope by file type. The `--f` pattern supports `|` for OR-matching. Add `--regex` for full regex.

### 4. Line Extraction (`--lines`)

```bash
src --lines "src/main.rs:1:20 src/cli.rs:18:40"
src --lines "src/scanner.rs:36:40" --lines "src/main.rs:19:22"
src --lines "src/scanner.rs:36:40" --line-numbers off
```

Pulls **exact line ranges** from files — no searching, no globbing. Specs are `path:start:end` (1-based, inclusive), space-separated within a single `--lines` value or across multiple `--lines` flags.

This is the "surgical extraction" mode. Use it when you already know the file and line range you need (e.g., from a previous search or a stack trace). Overlapping ranges on the same file are merged automatically.

**Cannot be combined** with `--f` or `--graph` (mutually exclusive).

### 5. Dependency Graph (`--graph`)

```bash
src --graph
src --graph --r "*.rs"
```

Emits a project-internal dependency graph — which source files import/reference which other source files, based on language-specific import/use statements. Only dependencies that resolve to files **within the project** are included; external packages are excluded.

Supported languages:
- **Rust** — `mod`, `use crate::`, `use super::`
- **TypeScript/JavaScript** — `import ... from './...'`, `require('./...')`, `export ... from './...'`
- **C#** — `using Namespace;` (namespace-to-path mapping, external namespaces filtered)

Use `--r` to scope the graph to specific file types. Files with no internal imports appear with `imports: []`.

**Cannot be combined** with `--f` or `--lines` (mutually exclusive).

## Key Options

| Option | Purpose |
|--------|---------|
| `--r <glob>` | Filter by file glob (repeatable) |
| `--f <pattern>` | Search pattern (`\|` for OR) |
| `--lines "<specs>"` | Extract exact line ranges (repeatable) |
| `--graph` | Emit source dependency graph |
| `--pad <n>` | Context lines around matches |
| `--line-numbers off` | Suppress per-line number prefixes in content output |
| `--regex` | Treat `--f` as regex |
| `-d <path>` | Set root directory |
| `--exclude <name>` | Add exclusion (repeatable) |
| `--no-defaults` | Disable built-in exclusions |
| `--timeout <secs>` | Execution time limit |

### Mutual Exclusions

`--f`, `--lines`, and `--graph` are mutually exclusive — only one of these three can be used per invocation.

## Recipes

### Gather full context from specific file types

Read every line of all matching files by matching everything with a high pad:

```bash
src --r "*.rs" --f "." --pad 999
```

This dumps the full contents of every `.rs` file with line numbers. The output key is `contents` (full file) or `chunks` (partial matches).

### Same, but without line numbers (cleaner for LLM context)

```bash
src --r "*.rs" --f "." --pad 999 --line-numbers off
```

Ideal when feeding raw code into an LLM prompt — no `36.  ` prefixes cluttering the content.

### Explore project structure first, then dive in

```bash
# Step 1: See what's here
src

# Step 2: Find relevant files
src --r "*.ts" --r "*.tsx"

# Step 3: Search for specific patterns with context
src --r "*.ts" --f "interface|type " --pad 3
```

### Pull exact line ranges from known files

```bash
src --lines "src/main.rs:1:20 src/cli.rs:18:40"
```

Use after a search told you where to look, or when reviewing specific line ranges from a code review or error trace.

### Pull exact lines without line numbers

```bash
src --lines "src/scanner.rs:36:40" --line-numbers off
```

### Multiple --lines flags

```bash
src --lines "src/main.rs:1:10" --lines "src/cli.rs:1:5"
```

Specs from all `--lines` flags are concatenated. Multiple ranges in the same file are merged.

### Find all public API surfaces in Rust

```bash
src --r "*.rs" --f "pub fn|pub struct|pub enum|pub trait" --pad 2
```

### Find all React components

```bash
src --r "*.tsx" --f "export default|export function" --pad 3
```

### Search across all source files for a term

```bash
src --f "PaymentService" --pad 3
```

Without `--r`, searches all non-excluded source files.

### Read specific file types fully

```bash
src --r "*.toml" --f "." --pad 999
src --r "*.json" --f "." --pad 999
```

### Understand project dependencies

```bash
src --graph
```

Shows which files depend on which. Great for understanding architecture before making changes.

### Scoped dependency graph

```bash
src --graph --r "*.rs"
src --graph --r "*.ts" --r "*.tsx"
```

Limit the graph to specific languages/file types.

### Scan a different directory

```bash
src -d ../other-project --r "*.py" --f "class " --pad 2
```

## Output Format

Output is always YAML. Key structures:

**Tree mode** — `tree.children[].name`, `tree.files[]`
**File list** — `files[].path`
**Search / Lines** — `files[].chunks[].startLine`, `files[].chunks[].endLine`, `files[].chunks[].content`
**Full file** — `files[].contents` (when the entire file matches or is requested)
**Graph** — `graph[].file`, `graph[].imports[]`

The `meta` block always includes `elapsedMs` and `filesMatched`. Search/graph modes also include `filesScanned`.

## When to Prefer src Over Other Tools

| Scenario | Use src | Use other tools |
|----------|---------|-----------------|
| Need context from 5+ files | Yes — single command | No — too many Read calls |
| Exploring unfamiliar codebase | Yes — tree + search | No |
| Finding files by pattern | Yes — `--r` with globs | Glob tool works too |
| Reading a single known file | No | Read tool is simpler |
| Exact symbol search in 1 file | No | Grep tool is simpler |
| Need to understand broad patterns | Yes — `--f` with `--pad` | No |
| Know exact file + line range | Yes — `--lines` | Read tool with offset/limit also works |
| Understanding module dependencies | Yes — `--graph` | No equivalent |
| Feeding clean code to LLMs | Yes — `--line-numbers off` | Manual cleanup |

## Tips

- **Batch your context gathering**: one `src --r "*.ts" --f "import|export" --pad 3` replaces dozens of individual file reads.
- **Start broad, then narrow**: tree first, then file list, then targeted search, then `--lines` for surgical reads.
- **Use `--pad` generously**: `--pad 5` gives you enough surrounding code to understand each match in context.
- **Use `--line-numbers off`** when the output will be fed into another tool or LLM — removes the `36.  ` prefixes for cleaner content.
- **Use `--lines` for follow-up reads**: after `--f` finds something interesting, grab the exact range with `--lines "file:start:end"`.
- **Use `--graph` before refactoring**: understanding which files depend on a module helps you assess the blast radius of changes.
- **Pipe to limit output**: for very large codebases, use `--timeout` or narrow `--r` patterns.
- **The `|` separator in `--f`** is a lightweight OR — no need for `--regex` for simple multi-term search.
