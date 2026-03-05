# Implementation Plan

#[[file:requirements.md]]
#[[file:design.md]]

- [x] 1. Add new CLI flags and update argument parsing
  - [x] 1.1 Add new fields to `CliArgs` in `src/cli.rs`
    - Add `callers: Option<String>`, `compact: bool`, `with_comments: bool`, `with_tests: bool`, `auto_expand: bool` to the struct
    - Initialize defaults in `parse_args`: `callers = None`, `compact = false`, `with_comments = false`, `with_tests = false`, `auto_expand = false`
    - _Requirements: 7.1, 7.3, 7.4, 7.5_

  - [x] 1.2 Add match arms for new flags in `parse_args`
    - `--callers` takes a required value (symbol name), stored in `callers`
    - `--compact` sets `compact = true`
    - `--with-comments` sets `with_comments = true`
    - `--with-tests` sets `with_tests = true`
    - `--auto-expand` sets `auto_expand = true`
    - _Requirements: 1.1, 2.1, 3.1, 4.1, 5.1, 6.1_

  - [x] 1.3 Update mutual exclusion logic in `parse_args`
    - Allow `--symbols` combined with `--find` (treat as single mode, remove the existing exclusion between them)
    - Add `--callers` to the exclusive set (mutually exclusive with `--find`, `--lines`, `--graph`, `--symbols`, `--count`, `--stats`)
    - Add post-parse validations: `--compact` requires `symbols`, `--with-comments` requires `symbols`, `--auto-expand` requires `lines`
    - Reject `--symbols --find --count` (count doesn't apply to symbol filtering)
    - _Requirements: 1.7, 1.8, 2.9, 3.2, 4.2, 6.2, 7.1, 7.2, 7.3, 7.4, 7.5_

  - [x] 1.4 Update `print_help` with new flag documentation
    - Add entries for `--callers <name>`, `--compact`, `--with-comments`, `--with-tests`, `--auto-expand` in both Modes and Options sections
    - Add examples: `src -s -f "handle" -g *.rs`, `src --callers process_file`, `src -s --compact`, `src -s --with-comments`, `src --lines "file:1:10" --auto-expand`
    - _Requirements: 7.6_

  - [x] 1.5 Write unit tests for all new CLI flags
    - Test each flag parses correctly, test mutual exclusion errors, test modifier-requires-parent errors, test `--symbols --find` is accepted, test `--callers` rejects combinations
    - _Requirements: 1.7, 1.8, 2.9, 3.2, 4.2, 6.2, 7.1-7.7_

- [x] 2. Implement test file filtering in the scanner
  - [x] 2.1 Add `is_test_file` function to `src/scanner.rs`
    - Match filename patterns: `*_test.*`, `*_spec.*`, `test_*.*`, `*.test.*`, `*.spec.*` (case-insensitive)
    - Match directory patterns: path component is `tests`, `test`, `__tests__`, `spec`, `specs`
    - Use the existing `glob::matches` function for pattern matching on the filename portion
    - _Requirements: 5.3_

  - [x] 2.2 Add `find_files_filtered` wrapper to `src/scanner.rs`
    - Wraps `find_files`, accepts `include_tests: bool` parameter
    - When `include_tests` is false, filter out results where `is_test_file` returns true
    - _Requirements: 5.1, 5.5_

  - [x] 2.3 Update all `execute_*` functions in `src/main.rs` to use `find_files_filtered`
    - Replace `scanner::find_files` calls with `scanner::find_files_filtered` in: `execute_file_listing`, `execute_search`, `execute_graph`, `execute_symbols`, `execute_stats`, `execute_count`
    - Pass `args.with_tests` as the `include_tests` parameter
    - Do NOT apply test filtering in `execute_lines` (explicit paths override, per Req 5.6)
    - Update `scan_directories` to accept `include_tests` and filter files during directory walking for tree mode
    - _Requirements: 5.1, 5.2, 5.4, 5.6, 5.7, 5.8_

  - [x] 2.4 Write unit tests for `is_test_file`
    - Test all filename patterns, directory patterns, case insensitivity, non-test files pass through
    - _Requirements: 5.3_

  - [x] 2.5 Write integration tests for `--with-tests` behavior
    - Add test fixture files that are test files, verify they're excluded by default, included with `--with-tests`
    - Test with `--symbols`, `--find`, `--stats`, `--graph`, tree mode
    - _Requirements: 5.1, 5.2, 5.4, 5.7_

- [x] 3. Implement symbol name filtering (`--symbols --find`)
  - [x] 3.1 Add `filter_symbols` function to `src/symbols.rs`
    - Takes `Vec<SymbolFile>` and `&Matcher`, returns `(Vec<SymbolFile>, usize)` where usize is total matching symbol count
    - Retains only symbols where `matcher.is_match(&sym.name)` is true
    - Drops files with zero remaining symbols (unless they have an error)
    - _Requirements: 1.1, 1.2, 1.3, 1.5_

  - [x] 3.2 Update `execute_symbols` in `src/main.rs` to handle combined mode
    - If `args.find.is_some()`, build a `Matcher` from the pattern (with `args.is_regex`)
    - After `symbols::extract_symbols`, call `symbols::filter_symbols`
    - Set `meta.total_matches` to the returned total count
    - Set `meta.files_matched` to the filtered file count
    - _Requirements: 1.1, 1.4, 1.5, 1.6, 1.9, 1.10_

  - [x] 3.3 Write unit tests for `filter_symbols`
    - Test literal match, multi-term match, regex match, no matches returns empty, files with errors preserved
    - _Requirements: 1.1, 1.2, 1.3, 1.6_

  - [x] 3.4 Write integration tests for `--symbols --find`
    - Test `src -s -f "main" -g *.rs`, test with regex, test with glob scoping, test no-match case
    - _Requirements: 1.1, 1.4, 1.9_

- [x] 4. Implement compact symbol output
  - [x] 4.1 Thread compact flag through `OutputEnvelope` to output layer
    - Add `pub compact_symbols: bool` to `OutputEnvelope` (default false)
    - Set it from `args.compact` in `execute_symbols`
    - _Requirements: 3.1_

  - [x] 4.2 Update `write_symbols` in `src/yaml_output.rs` for compact YAML
    - When compact is true, skip kind-group headers, emit flat list: `kind name :line:end_line`
    - Omit signature, visibility, parent fields
    - _Requirements: 3.1, 3.3_

  - [x] 4.3 Update `write_symbols_json` in `src/yaml_output.rs` for compact JSON
    - When compact is true, emit only `kind`, `name`, `line`, `endLine` per symbol object
    - _Requirements: 3.4_

  - [x] 4.4 Write unit tests for compact output
    - Test YAML compact format, JSON compact format, compact with `--find` filter
    - _Requirements: 3.1, 3.4, 3.5_

  - [x] 4.5 Write integration tests for `--symbols --compact`
    - Test `src -s --compact -g *.rs`, verify output format, test with `--json`
    - _Requirements: 3.1, 3.4, 3.5_

- [x] 5. Implement doc comment extraction
  - [x] 5.1 Add `comment: Option<String>` field to `SymbolInfo` in `src/lang/mod.rs`
    - Default to `None` in all existing symbol construction sites across all 8 language handlers
    - _Requirements: 4.3, 4.10_

  - [x] 5.2 Implement `extract_preceding_comment` in `src/lang/common.rs`
    - Walk backwards from `symbol_line_idx - 1`, collect contiguous comment lines
    - Strip comment markers: `///`, `//!`, `//`, `/*`, `*/`, `*`, `#`
    - Stop at blank line, non-comment code line, or file start
    - Return `Option<String>` (None if no comment found)
    - _Requirements: 4.1, 4.4, 4.5, 4.6, 4.7_

  - [x] 5.3 Implement `extract_docstring_after` in `src/lang/common.rs`
    - For Python: scan lines starting at `symbol_line_idx + 1` for `"""..."""` or `'''...'''` docstrings
    - Strip triple-quote delimiters, return content
    - _Requirements: 4.6_

  - [x] 5.4 Add comment post-processing step in `src/symbols.rs`
    - After `handler.extract_symbols(&content)` returns, if `with_comments` is true:
    - Split content into lines, for each symbol call `extract_preceding_comment(lines, sym.line - 1)`
    - For Python files (detect by extension), also try `extract_docstring_after` if preceding comment is empty
    - Populate `sym.comment` with the result
    - Thread `with_comments: bool` through `extract_symbols` and `process_file`
    - _Requirements: 4.1, 4.3, 4.10_

  - [x] 5.5 Update YAML output for comments in `src/yaml_output.rs`
    - In `write_symbol_compact` (non-compact mode): if `sym.comment.is_some()`, emit a `comment:` block scalar after the signature line
    - In compact mode with comments: emit `comment:` after the compact `kind name :line:end` line
    - _Requirements: 4.4, 4.8_

  - [x] 5.6 Update JSON output for comments in `src/yaml_output.rs`
    - In `write_symbols_json`: if `sym.comment.is_some()`, add `"comment":"..."` field
    - _Requirements: 4.9_

  - [x] 5.7 Write unit tests for comment extraction
    - Test Rust `///` comments, C-style `/** */`, Python docstrings, `//` comments, `#` comments
    - Test no-comment case, blank line stops walking, multiple comment styles
    - _Requirements: 4.4, 4.5, 4.6, 4.7_

  - [x] 5.8 Write integration tests for `--with-comments`
    - Add fixture files with doc comments, verify output includes comments, verify default excludes them
    - Test with `--compact --with-comments`, test with `--json`
    - _Requirements: 4.1, 4.3, 4.8, 4.9_

- [x] 6. Implement callers mode
  - [x] 6.1 Add caller data models to `src/models.rs`
    - Add `CallerEntry` (path, line, content), `CallerFile` (path, sites), `CallerDeclaration` (path, line, signature), `CallersOutput` (declarations, files)
    - Add `pub callers: Option<CallersOutput>` to `OutputEnvelope`
    - Update `Default` impl for `OutputEnvelope`
    - _Requirements: 2.3, 2.4_

  - [x] 6.2 Create `src/callers.rs` module with core logic
    - Implement `find_callers(file_paths, root, name, is_regex, cancelled)` returning `CallersOutput`
    - Step 1: Extract symbols from all files, find declarations matching the name
    - Step 2: Build a `Matcher` for the name, search all files for matching lines
    - Step 3: Exclude declaration lines from results (match by path + line number)
    - Step 4: Group references by file, sort by path
    - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.7, 2.8_

  - [x] 6.3 Add `execute_callers` function to `src/main.rs`
    - Wire up: `find_files_filtered` → `callers::find_callers` → build `OutputEnvelope` → `emit`
    - Handle `--glob` scoping, `--regex`, `--limit`, `--timeout`
    - Set `meta.totalMatches` to total reference count
    - _Requirements: 2.1, 2.5, 2.6, 2.10, 2.11, 2.12_

  - [x] 6.4 Update dispatch chain in `execute` to include callers
    - Insert `callers` check after `graph` and before `symbols` in the priority waterfall
    - _Requirements: 7.7_

  - [x] 6.5 Add YAML output for callers in `src/yaml_output.rs`
    - Write `declaration:` section (or `declaration: null`), then `callers:` list grouped by file
    - Each caller entry: `path`, `line`, `content`
    - _Requirements: 2.3, 2.4, 2.8_

  - [x] 6.6 Add JSON output for callers in `src/yaml_output.rs`
    - Same structure in JSON format
    - _Requirements: 2.3, 2.4_

  - [x] 6.7 Register `mod callers` in `src/main.rs`
    - Add `mod callers;` to the module declarations
    - _Requirements: 2.1_

  - [x] 6.8 Write unit tests for `find_callers`
    - Test finding declaration + references, test no-declaration case, test no-references case, test regex name matching
    - _Requirements: 2.1, 2.2, 2.7, 2.8_

  - [x] 6.9 Write integration tests for `--callers`
    - Test `src --callers main`, test with `--glob`, test mutual exclusion errors, test JSON output
    - _Requirements: 2.1, 2.5, 2.9, 2.10_

- [x] 7. Implement auto-expand for line ranges
  - [x] 7.1 Add `expand_line_specs` function to `src/lines.rs`
    - For each spec, read the target file, get symbol handler, extract symbols
    - Find the enclosing symbol (the one whose `[start, end]` range contains `spec.start`)
    - Expand spec to `[symbol.start, max(symbol.end, spec.end)]`
    - If `with_comments` is true, walk backwards from symbol start to include preceding comment block
    - If no enclosing symbol or unsupported extension, return spec unchanged
    - Clamp expanded end to file's total line count
    - _Requirements: 6.1, 6.4, 6.5, 6.6, 6.7, 6.8, 6.10_

  - [x] 7.2 Wire auto-expand into `execute_lines` in `src/main.rs`
    - After `lines::parse_line_specs`, if `args.auto_expand`, call `lines::expand_line_specs` on the parsed specs
    - Pass `args.with_comments` for comment-inclusive expansion
    - Feed expanded specs to `lines::extract_lines` (rest of pipeline unchanged)
    - _Requirements: 6.1, 6.3_

  - [x] 7.3 Write unit tests for `expand_line_specs`
    - Test expansion within a function, test range spanning multiple symbols (no expansion), test unsupported extension (no expansion), test with comments, test clamping
    - _Requirements: 6.1, 6.4, 6.5, 6.6, 6.7, 6.8, 6.10_

  - [x] 7.4 Write integration tests for `--auto-expand`
    - Use fixture files, request a range inside a function, verify output includes the full function
    - Test without `--auto-expand` for comparison, test with `--with-comments --auto-expand`
    - _Requirements: 6.1, 6.3, 6.8_

- [x] 8. Final integration wiring and output tests
  - [x] 8.1 Update `write_envelope_yaml` and `write_envelope_json` for callers output
    - Add the `callers` branch to the envelope writing logic, after `symbols` and before `counts`
    - _Requirements: 2.3, 2.4_

  - [x] 8.2 Run full test suite and fix any regressions
    - `cargo test` — all 662+ existing tests must pass
    - Verify new tests pass
    - Verify mutual exclusion error messages are clear and list flag names
    - _Requirements: 7.1, 7.2_

  - [x] 8.3 Update `README.md` with new flags and examples
    - Add `--callers`, `--compact`, `--with-comments`, `--with-tests`, `--auto-expand` to the Options table
    - Add examples for combined modes: `src -s -f "handle"`, `src --callers main`, `src -s --compact`, etc.
    - _Requirements: 7.6_
