# src

`src` is a fast, single-binary CLI for interrogating source code in parallel.
It is built for agent workflows and for engineers who want structured answers instead of stitching together `find`, `grep`, `cat`, and ad hoc scripts.

The output is YAML by default, JSON when requested, and designed to be easy for both humans and tools to consume.

## Why This Exists

Most codebase inspection ends up looking like this:

```bash
find . -name "*.ts"
grep -R "createInvoice" .
sed -n '120,180p' src/payments/service.ts
sed -n '40,90p' src/orders/controller.ts
```

`src` collapses that into one tool with a consistent output shape:

- project tree for fast orientation
- globbed file lists
- full-file search results or scoped context windows
- exact multi-file line extraction
- internal dependency graphs
- symbol extraction across supported languages
- caller tracing for a symbol name
- codebase stats for sizing and hotspot detection

## Install

```bash
cargo install --path .
```

Or build locally:

```bash
cargo build --release
# binary at target/release/src
```

## Quick Start

```bash
src
src -g "*.rs"
src -f "TODO|FIXME"
src -f "createInvoice|finalizeInvoice" -g "*.ts" -c
src --lines "src/main.rs:1:40 src/cli.rs:220:293"
src --graph -g "*.tsx" -g "*.ts"
src --symbols -g "*.rs" --compact
src --callers process_file -g "*.rs"
src --stats
```

## Real Workflows

### 1. Triage a real TypeScript app before opening files

Count where a state hook or factory shows up:

```bash
src -d U:\Users\source\churchofchristapp\ui \
  -g "*.ts" -g "*.tsx" \
  -f "useMemberStore|create" \
  -c -L 8
```

Actual output:

```yaml
meta:
  elapsedMs: 26
  filesScanned: 16
  filesMatched: 4
  totalMatches: 16
files:
- path: src/api/memberApiClient.ts
  count: 2
- path: src/main.tsx
  count: 2
- path: src/screens/ClassesScreen.tsx
  count: 6
- path: src/stores/memberAppStore.ts
  count: 6
```

This is the fast first pass before reading anything.

### 2. Expand one line into the full enclosing function

Start from a single line number and pull the whole symbol:

```bash
src --lines "src/main.rs:185:185" --auto-expand
```

Actual output:

```yaml
meta:
  elapsedMs: 1
  filesMatched: 1
files:
- path: src/main.rs
  chunks:
  - startLine: 157
    endLine: 203
    content: |
      157.  fn execute(args: cli::CliArgs) -> i32 {
      158.      let root = Path::new(&args.root);
      159.      let format = resolve_format(&args);
      ...
      184.      if !args.lines.is_empty() {
      185.          execute_lines(&args, root, &cancelled, start, format)
      186.      } else if args.graph {
      ...
      203.  }
```

This is useful when another tool or stack trace only gives you a line number.

### 3. Find declarations and call sites for a symbol

Trace a helper through the codebase:

```bash
src --callers process_file -g "*.rs"
```

Actual output:

```yaml
meta:
  elapsedMs: 9
  filesScanned: 27
  filesMatched: 8
  totalMatches: 11
declarations:
- path: src/count.rs
  line: 33
  signature: "fn process_file(file_path: &str, root: &Path, matcher: &Matcher) -> Option<CountEntry> {"
- path: src/graph.rs
  line: 38
  signature: fn process_file(
callers:
- path: src/count.rs
  sites:
  - line: 23
    content: process_file(file_path, root, matcher)
- path: src/searcher.rs
  sites:
  - line: 96
    content: process_file(file_path, root, matcher, line_numbers, context)
```

This is the quickest way to answer "where is this declared, and who actually uses it?"

### 4. Search with context windows instead of dumping full files

When a full file is too noisy:

```bash
src -f "with_comments|with_tests" -g "*.rs" -C 2 -L 2
```

Actual output:

```yaml
files:
- path: src/cli.rs
  chunks:
  - startLine: 143
    endLine: 148
    content: |
      143.              }
      144.              "--compact" => compact = true,
      145.              "--with-comments" => with_comments = true,
      146.              "--with-tests" => with_tests = true,
      147.              "--auto-expand" => auto_expand = true,
      148.              "--output" | "-o" => {
```

Use `-C` when you want grep-like focus but still need structured output.

### 5. Map internal dependencies with `--graph`

Use the graph when you need to understand which files depend on which local modules:

```bash
src --graph -g "*.rs" -L 8
```

Actual output:

```yaml
meta:
  elapsedMs: 4
  filesScanned: 27
  filesMatched: 8
graph:
- file: src/alias.rs
  imports:
  - src/file_reader.rs
- file: src/callers.rs
  imports:
  - src/file_reader.rs
  - src/models.rs
  - src/path_helper.rs
  - src/searcher.rs
  - src/symbols.rs
- file: src/count.rs
  imports:
  - src/file_reader.rs
  - src/models.rs
  - src/path_helper.rs
  - src/searcher.rs
```

This is useful before changing a shared module because it shows internal coupling without external package noise.

### 6. Scan declarations with `--symbols`

Use symbols to get the public shape of files before reading implementations:

```bash
src --symbols -g "*.rs" --compact -L 5
```

Actual output:

```yaml
meta:
  elapsedMs: 4
  filesScanned: 27
  filesMatched: 5
symbols:
- path: src/callers.rs
  - fn find_callers :13:99
- path: src/cli.rs
  - struct CliArgs :2:25
  - enum OutputFormatArg :28:31
  - enum CliAction :34:38
  - fn parse_args :40:219
  - fn print_help :221:293
- path: src/count.rs
  - fn count_matches :11:31
  - fn process_file :33:51
```

This gives you the outline first: file, declaration kind, name, and line range.

### 7. Roll multiple file reads into one command

Instead of three separate file reads, batch exact ranges into one `--lines` call:

```bash
src --lines "src/main.rs:157:203 src/cli.rs:221:293 src/models.rs:95:116"
```

Actual output shape:

```yaml
meta:
  filesMatched: 3
files:
- path: src/cli.rs
  chunks:
  - startLine: 221
    endLine: 293
    content: |
      221.  pub fn print_help() {
      ...
- path: src/main.rs
  chunks:
  - startLine: 157
    endLine: 203
    content: |
      157.  fn execute(args: cli::CliArgs) -> i32 {
      ...
- path: src/models.rs
  chunks:
  - startLine: 95
    endLine: 116
    content: |
      95.  pub enum OutputPayload {
      ...
```

This is the core agent workflow: one command returns several focused source ranges in a stable, structured response.

## Modes

| Mode | Command | What it returns |
|------|---------|-----------------|
| Tree | `src` | Directory hierarchy of source files |
| Glob | `src -g "*.ts"` | Flat file list |
| Find | `src -f "auth|token"` | Matching files with full contents |
| Find with context | `src -f "auth|token" -C 3` | Matching files with focused chunks |
| Count | `src -f "auth|token" -c` | Match counts per file |
| Lines | `src --lines "a.rs:1:30 b.ts:40:90"` | Exact ranges from multiple files |
| Lines auto-expand | `src --lines "a.rs:88:88" --auto-expand` | Full enclosing symbol for the referenced line |
| Graph | `src --graph` | Project-internal dependency/import map |
| Symbols | `src --symbols -g "*.rs"` | Symbol declarations with ranges |
| Compact symbols | `src --symbols --compact` | Condensed declaration listing |
| Callers | `src --callers handleAuth` | Declarations plus call sites |
| Stats | `src --stats` | File, line, byte, and hotspot summary |

## Flags That Matter In Practice

| Flag | Meaning |
|------|---------|
| `--dir`, `-d <path>` | Scan another repo without changing directories |
| `--glob`, `-g <pattern>` | Restrict by file pattern; repeatable |
| `--find`, `-f <pattern>` | Search contents; `|` works as a literal OR |
| `--regex`, `-E` | Treat `--find` as regex |
| `--count`, `-c` | Return counts instead of file contents |
| `--context`, `-C <n>` | Return match windows instead of full files |
| `--lines "<specs>"` | Extract exact file ranges in one call |
| `--auto-expand` | Expand a `--lines` location to the enclosing symbol |
| `--graph` | Build an internal dependency graph |
| `--symbols`, `-s` | Extract declarations |
| `--compact` | Condense symbol output for scanning |
| `--with-comments` | Include doc comments in symbol output |
| `--with-tests` | Include test files normally skipped by source scanning |
| `--callers <name>` | Find declaration(s) and call sites for a symbol |
| `--limit`, `-L <n>` | Cap result size |
| `--json` | Emit JSON instead of YAML |
| `--output`, `-o <path>` | Save results as an artifact |

## Output Shape

YAML is the default because it is readable and works well for LLM pipelines:

```yaml
meta:
  elapsedMs: 16
  filesScanned: 60
  filesMatched: 28
  totalMatches: 44
files:
- path: src/payments/service.ts
  chunks:
  - startLine: 118
    endLine: 132
    content: |
      118.  export async function createInvoice(...) {
      ...
```

Switch to JSON when you want to pipe the results somewhere else:

```bash
src --symbols -g "*.rs" --json
src --stats -o stats.yaml
```

## Supported Languages

Import resolution and symbol extraction currently support:

- Rust
- TypeScript / JavaScript
- C#
- Go
- Java
- Kotlin
- Ruby
- Python

Other file types still work with tree, glob, find, lines, and stats modes.

## Recommended Patterns

Use these in order when you are dropped into an unfamiliar repo:

1. `src --stats`
2. `src --graph -g "*.ts" -g "*.tsx"` or `src --graph -g "*.rs"`
3. `src --symbols --compact -g "*.ts" -g "*.tsx"` or `src --symbols --compact -g "*.rs"`
4. `src -f "termA|termB" -c`
5. `src --lines "file:line:line" --auto-expand`
6. `src --callers symbolName`

That sequence gets you from orientation to exact code with very little waste.

## Architecture

`src` is implemented in Rust and keeps the runtime simple:

- `rayon` for parallel scanning
- `memmap2` for fast file access on larger files
- `regex` for regex search mode
- `memchr` for fast newline counting

The current codebase is roughly 14.5k lines, with language handlers split by platform-specific parser in [`src/lang/`](./src/lang).

## Tests

```bash
cargo test
```

## License

MIT
