# Requirements Document

## Introduction

We're adding six enhancements to `src` that make the `--symbols` mode dramatically more useful for code exploration — both for AI agents and developers. Right now symbols give you declarations with line ranges, but you can't search them, can't pull their doc comments, can't auto-expand to full bodies, and can't control whether test code is included. These features close those gaps and turn `--symbols` from an index into a full code understanding pipeline.

The six features are:

1. **`--symbols --find <pattern>`** — Symbol name filtering. Lifts the mutual exclusion between `--symbols` and `--find` so you can search within symbols. Returns only symbols whose name matches the pattern, keeping the same compact output. Effectively symbol-level grep.

2. **`--callers <name>`** — Cross-reference mode. Given a symbol name, finds all files and lines that reference it. Combines the symbol index (to know what's declared) with text search (to know where it's called). Returns a structured `callers:` section with per-file call sites.

3. **`--compact`** — Ultra-compact symbol output. Strips signatures down to just `kind name :line:end_line` per symbol, one line each, grouped by file. Intended for when you need a quick index and don't care about full signatures. Works as a modifier on `--symbols`.

4. **`--with-comments`** — Include doc comments. When enabled with `--symbols`, extracts the comment block immediately preceding each declaration and attaches it to the symbol entry. Defaults to false. Gives AI agents the ability to read intent alongside structure.

5. **`--with-tests`** — Include test files in output. By default `src` modes include test files. This flag defaults to false and when explicitly set to true, confirms test file inclusion. When false, test files (files matching `*_test.*`, `*_spec.*`, `test_*.*`, `tests/**`, `__tests__/**`, `spec/**`) are excluded. This effectively inverts the current behavior: tests are excluded by default, `--with-tests` opts them back in.

6. **`--auto-expand`** — Automatic line range expansion. When `--lines` targets a range that starts inside a function body, automatically expands the range to include the full enclosing symbol (using symbol data to determine bounds). Defaults to false. When enabled, you never accidentally clip a function in half.

All six integrate with existing flags (`--glob`, `--dir`, `--timeout`, `--exclude`, `--no-defaults`, `--limit`, `--format`, `--output`) and emit structured YAML/JSON through the existing `OutputEnvelope` + `yaml_output` pipeline.

## Requirements

### Requirement 1: Symbol Name Filtering (`--symbols --find <pattern>`)

**User Story:** As a developer or LLM agent, I want to search for symbols by name pattern, so that I can quickly find specific functions, classes, or types across a codebase without reading every file's full symbol listing.

#### Acceptance Criteria

1. WHEN the user provides both `--symbols` and `--find <pattern>` THEN the system SHALL extract symbols from all matched files and return only symbols whose `name` field matches the pattern.
2. WHEN `--symbols --find` is used THEN the matching SHALL be case-insensitive by default, consistent with existing `--find` behavior.
3. WHEN `--symbols --find` is combined with `--regex` / `-E` THEN the system SHALL treat the find pattern as a regular expression applied to symbol names.
4. WHEN `--symbols --find` is combined with `--glob` / `-g` THEN the system SHALL scope symbol extraction to files matching the glob patterns before applying the name filter.
5. WHEN `--symbols --find` is used THEN the system SHALL use the existing `Matcher` (from `searcher.rs`) to perform the matching, reusing the literal/multi-term/regex pipeline.
6. WHEN no symbols match the pattern THEN the system SHALL return `symbols: []` with `meta.filesMatched: 0`.
7. WHEN `--symbols --find` is used THEN the mutual exclusion rule SHALL be updated: `--symbols` combined with `--find` is now permitted; `--find` (without `--symbols`) and `--find --count` remain mutually exclusive with `--symbols`.
8. WHEN `--symbols --find <pattern>` is combined with `--count` THEN the system SHALL reject with a mutual exclusion error (count applies to content search, not symbol search).
9. WHEN the `meta` block is emitted for `--symbols --find` THEN it SHALL include `filesScanned`, `filesMatched` (files that had at least one matching symbol), and `totalMatches` (total number of symbols matching).
10. WHEN `--symbols --find` is combined with `--limit` THEN `--limit` SHALL cap the number of files in the output, not the number of symbols.

### Requirement 2: Cross-Reference / Callers Mode (`--callers <name>`)

**User Story:** As a developer or LLM agent, I want to find all call sites and references to a given symbol, so that I can understand how a function, class, or type is used across the codebase without manually grepping.

#### Acceptance Criteria

1. WHEN the user provides `--callers <name>` THEN the system SHALL search all non-excluded source files for references to the given name.
2. WHEN `--callers` is used THEN the system SHALL first extract symbols to identify the declaration site(s) of the name, then search all files for lines containing the name, excluding the declaration line itself.
3. WHEN `--callers` results are emitted THEN each entry SHALL include: `path` (relative file path), `line` (1-based line number), and `content` (the line text, trimmed).
4. WHEN `--callers` results are emitted THEN the output SHALL be grouped under a `callers:` section, with a `declaration:` section listing where the symbol is defined (path, line, signature).
5. WHEN `--callers` is combined with `--glob` / `-g` THEN the system SHALL scope the reference search to files matching the glob patterns.
6. WHEN `--callers` is combined with `--regex` / `-E` THEN the name SHALL be treated as a regex pattern for matching references.
7. WHEN no references are found THEN the system SHALL return `callers: []` with the declaration still listed (if found).
8. WHEN no declaration is found for the name THEN the system SHALL still search for references (the symbol might be from an external dependency) and emit `declaration: null`.
9. WHEN `--callers` is combined with `--find`, `--lines`, `--graph`, `--symbols`, `--count`, or `--stats` THEN the system SHALL reject with a mutual exclusion error.
10. WHEN `--callers` is used THEN the `meta` block SHALL include `totalMatches` (total reference count across all files).
11. WHEN `--limit` is used with `--callers` THEN it SHALL cap the number of files listed in the callers output.
12. WHEN `--timeout` is active during `--callers` THEN the system SHALL return partial results with `meta.timeout: true`.

### Requirement 3: Compact Symbol Output (`--compact`)

**User Story:** As a developer or LLM agent, I want an ultra-compact symbol listing that shows only kind, name, and line range per symbol, so that I can get a quick index of a codebase with minimal output volume.

#### Acceptance Criteria

1. WHEN the user provides `--symbols --compact` THEN the system SHALL emit symbols in a minimal format: `kind name :line:end_line` per symbol, one per line, grouped under each file path.
2. WHEN `--compact` is used without `--symbols` THEN the system SHALL reject with a clear error: `"--compact requires --symbols"`.
3. WHEN `--compact` is used THEN signatures, visibility, and parent fields SHALL be omitted from the output.
4. WHEN `--compact` is used with `--format json` THEN the JSON output SHALL include only `kind`, `name`, `line`, and `endLine` per symbol (no `signature`, `visibility`, or `parent`).
5. WHEN `--compact` is combined with `--symbols --find` THEN the compact format SHALL apply to the filtered symbol results.
6. WHEN `--compact` is combined with `--with-comments` THEN comments SHALL still be included (compact strips signatures, not comments).

### Requirement 4: Doc Comment Extraction (`--with-comments`)

**User Story:** As a developer or LLM agent, I want to optionally include doc comments for each symbol, so that I can read the intent and documentation alongside the structural metadata.

#### Acceptance Criteria

1. WHEN the user provides `--symbols --with-comments` THEN the system SHALL extract the contiguous comment block immediately preceding each symbol declaration.
2. WHEN `--with-comments` is used without `--symbols` THEN the system SHALL reject with a clear error: `"--with-comments requires --symbols"`.
3. WHEN `--with-comments` is not provided THEN symbol entries SHALL NOT include comment data (default behavior unchanged).
4. WHEN a doc comment is found THEN the symbol entry SHALL include a `comment` field containing the full comment text with leading comment markers (`///`, `//`, `#`, `/** */`, etc.) stripped.
5. WHEN a symbol has no preceding comment block THEN the `comment` field SHALL be omitted from that symbol entry.
6. WHEN extracting comments THEN the system SHALL recognize: Rust `///` and `//!` doc comments, C-style `/** */` block comments, Python `"""docstrings"""` (immediately after a `def`/`class`), `//` line comments, and `#` line comments.
7. WHEN extracting comments THEN the system SHALL stop walking backwards when encountering a blank line, a non-comment code line, or the start of the file.
8. WHEN `--with-comments` is used with `--compact` THEN comments SHALL be included (compact only strips signatures, not comments).
9. WHEN `--with-comments` is used with `--format json` THEN the `comment` field SHALL be included as a JSON string in each symbol object.
10. WHEN the comment extraction is implemented THEN it SHALL be added to the `LangSymbols` trait as an optional method with a default implementation, so existing language handlers work without modification.

### Requirement 5: Test File Inclusion Control (`--with-tests`)

**User Story:** As a developer or LLM agent, I want test files excluded from output by default, so that I see production code only, and I can opt in to including tests when I need them.

#### Acceptance Criteria

1. WHEN `--with-tests` is NOT provided THEN the system SHALL exclude files matching test patterns from all output modes (tree, glob, find, symbols, graph, stats, count, lines, callers).
2. WHEN `--with-tests` is provided THEN the system SHALL include test files in output, restoring the current (pre-change) behavior.
3. WHEN test files are filtered THEN the system SHALL match against the following patterns (case-insensitive): `*_test.*`, `*_spec.*`, `test_*.*`, files in directories named `tests`, `test`, `__tests__`, `spec`, `specs`, and files matching `*.test.*`, `*.spec.*`.
4. WHEN `--with-tests` is used THEN it SHALL work with all modes: `--symbols`, `--find`, `--graph`, `--stats`, `--count`, `--callers`, `--lines`, tree, and glob.
5. WHEN the test file filter is applied THEN it SHALL operate at the file-path level in the scanner, after glob matching and exclusion filtering.
6. WHEN `--lines` is used with explicit file paths that are test files THEN the system SHALL still include them regardless of `--with-tests` (explicit paths override the filter).
7. WHEN `--stats` is used without `--with-tests` THEN test files SHALL be excluded from the statistics (both line counts and file counts).
8. WHEN `--with-tests` is used THEN it SHALL have no interaction with `--exclude` or `--no-defaults` (they are independent filters).

### Requirement 6: Auto-Expand Line Ranges (`--auto-expand`)

**User Story:** As a developer or LLM agent, I want `--lines` to automatically expand partial ranges to include the full enclosing function or type, so that I never accidentally clip a declaration in half.

#### Acceptance Criteria

1. WHEN the user provides `--lines <specs> --auto-expand` THEN the system SHALL expand each line range to include the full enclosing symbol if the range starts inside a symbol body.
2. WHEN `--auto-expand` is used without `--lines` THEN the system SHALL reject with a clear error: `"--auto-expand requires --lines"`.
3. WHEN `--auto-expand` is not provided THEN `--lines` SHALL behave exactly as it does today (no expansion).
4. WHEN auto-expansion is applied THEN the system SHALL use symbol extraction (same as `--symbols`) on the target file to determine symbol boundaries (start line to end line).
5. WHEN a requested line range falls entirely within a symbol's body (between its start line and end line) THEN the system SHALL expand the range to `[symbol.start_line, max(symbol.end_line, original_end_line)]`.
6. WHEN a requested line range spans multiple symbols or starts outside any symbol THEN the system SHALL NOT expand that range (leave it as-is).
7. WHEN the file has no symbol handler (unsupported extension) THEN auto-expand SHALL fall back to no expansion for that file.
8. WHEN `--auto-expand` is combined with `--with-comments` THEN the expansion SHALL also include the doc comment block preceding the enclosing symbol.
9. WHEN `--timeout` is active during auto-expand THEN the system SHALL return partial results with `meta.timeout: true`.
10. WHEN the expanded range would exceed the file's total line count THEN the system SHALL clamp to the file's last line.

### Requirement 7: CLI Integration and Mutual Exclusion Updates

**User Story:** As a user of `src`, I want clear and consistent behavior when combining new flags with existing ones, so that I never get confusing or silent failures.

#### Acceptance Criteria

1. WHEN the updated mutual exclusion rules are enforced THEN the following modes SHALL be mutually exclusive: `--find` (without `--symbols` or `--count`), `--find --count`, `--lines`, `--graph`, `--symbols` (with or without `--find`), `--stats`, `--callers`.
2. WHEN `--symbols --find` is used THEN it SHALL be treated as a single combined mode, not as two separate modes.
3. WHEN `--compact` is provided without `--symbols` THEN the system SHALL emit: `"--compact requires --symbols"`.
4. WHEN `--with-comments` is provided without `--symbols` THEN the system SHALL emit: `"--with-comments requires --symbols"`.
5. WHEN `--auto-expand` is provided without `--lines` THEN the system SHALL emit: `"--auto-expand requires --lines"`.
6. WHEN `--help` is displayed THEN it SHALL include entries for all new flags with descriptions and examples.
7. WHEN the dispatch priority is evaluated THEN the order SHALL be: `--lines` > `--graph` > `--callers` > `--symbols` (with or without `--find`) > `--stats` > `--find --count` > `--find` > `--glob` only > default tree.
