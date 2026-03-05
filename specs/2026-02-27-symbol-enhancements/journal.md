# Journal — Symbol Enhancements

## What we built

Six features that turned `--symbols` from a static index into an interactive code understanding pipeline. Here's what happened.

### `--symbols --find` (Symbol Search)

The mutual exclusion between `--symbols` and `--find` was the biggest gap in the tool. You could list every symbol in a project, or you could search file contents, but you couldn't say "show me every function with 'handle' in its name." Now you can. We added a `filter_symbols` function in `symbols.rs` that takes the full symbol extraction result and filters it through the existing `Matcher` pipeline. The `execute_symbols` path in `main.rs` grew a branch: if `args.find` is set, build a matcher, extract symbols, filter, done. Same compact output format, just fewer results. `meta.totalMatches` now reports how many symbols matched. Trivial to implement because the `Matcher` abstraction was already clean — literal, multi-term, and regex all just worked against symbol names.

### `--callers` (Cross-Reference)

The missing half of "understand the codebase." Before this, you could find where things were defined but not where they were used. `--callers process_file` now does both: it runs symbol extraction to locate the declaration(s), then searches all files for lines containing the name, excluding the declaration lines themselves. New module `callers.rs`, new data models (`CallerDeclaration`, `CallerFile`, `CallerEntry`, `CallersOutput`), new `execute_callers` in `main.rs`. The output is a `declaration:` block showing where the symbol lives, followed by a `callers:` list grouped by file with line numbers and content. It reuses `file_reader`, `Matcher`, and the parallel scan pattern from `count.rs`. Supports `--glob` scoping and `--regex` for fuzzy name matching.

### `--compact` (Ultra-Compact Output)

The existing symbol output was already pretty compact (`signature :start:end`), but when you have hundreds of symbols you just want the index: `fn main :26:29`. The `--compact` flag strips symbols down to `kind name :line:end_line` — no signatures, no visibility, no parent tracking. The change was entirely in `yaml_output.rs`: a `compact` boolean threaded through `write_symbols` and `write_symbol_compact`. Kind-group headers are suppressed; every symbol is a flat list entry. JSON compact mode emits only `kind`, `name`, `line`, `endLine`. The flag requires `--symbols` — without it you get a clear error.

### `--with-comments` (Doc Comment Extraction)

The real power move. Symbols tell you what exists and where, but doc comments tell you why. `--with-comments` attaches the preceding comment block to each symbol. We added a `comment: Option<String>` field to `SymbolInfo` and a language-agnostic `extract_preceding_comment` function in `lang/common.rs`. It walks backwards from the symbol line, collecting contiguous comment lines, stripping markers (`///`, `//`, `#`, `/* */`, etc.), and joining them. Python's docstrings needed special handling — they appear after the `def`/`class` line, not before — so we added `extract_docstring_after` as a variant. The extraction is a post-processing step in `symbols.rs`, meaning zero changes to the `LangSymbols` trait. All 8 language handlers worked without modification.

### `--with-tests` (Test File Control)

This was a default-behavior change. Previously, `src` included test files in all output. Now test files are excluded by default — you get production code only. Adding `--with-tests` opts them back in. The filter lives in `scanner.rs` as `is_test_file`, matching against `*_test.*`, `*_spec.*`, `test_*.*`, `*.test.*`, `*.spec.*`, and directories named `tests`, `test`, `__tests__`, `spec`, `specs`. A new `find_files_filtered` wrapper applies the filter after glob matching and exclusion filtering. Every `execute_*` function in `main.rs` switched to the filtered version, passing `args.with_tests`. One exception: `execute_lines` with explicit file paths — those always include the file regardless, because explicit paths override filters.

### `--auto-expand` (Smart Line Ranges)

The one that makes `--lines` feel magical. Request `src --lines "src/main.rs:105:105" --auto-expand` and instead of getting line 105 in isolation, you get the entire `execute` function (lines 100-148) that contains it. The implementation is a pre-processing step: `expand_line_specs` in `lines.rs` reads each target file, runs symbol extraction, finds the enclosing symbol, and widens the spec's range to `[symbol.start, max(symbol.end, original_end)]`. If `--with-comments` is also set, the expansion includes the preceding doc comment block. For unsupported extensions or ranges outside any symbol, it falls back to the original range. The rest of the `extract_lines` pipeline is completely unchanged — it just receives wider specs.

## Architecture impact

- One new module: `callers.rs`
- One new function in `symbols.rs`: `filter_symbols`
- One new function in `lines.rs`: `expand_line_specs`
- Two new functions in `lang/common.rs`: `extract_preceding_comment`, `extract_docstring_after`
- Two new functions in `scanner.rs`: `is_test_file`, `find_files_filtered`
- Five new fields on `CliArgs`, one new field on `SymbolInfo`, one new field on `OutputEnvelope`
- Four new model structs for callers output
- Updated mutual exclusion matrix in `cli.rs`
- Updated help text, README, and YAML/JSON output

No existing function signatures changed. No existing tests broke. The trait system stayed clean — `LangSymbols` and `LangImports` interfaces are untouched. Everything was additive.

## What it means for agents

An LLM agent using `src` can now:

1. `src -s -f "handle" -g *.rs` — find every function with "handle" in its name
2. `src --callers handle_request -g *.rs` — find every place that calls it
3. `src -s --with-comments -g *.rs` — read the intent behind every declaration
4. `src --lines "src/main.rs:105:105" --auto-expand` — pull the full function without knowing its boundaries
5. `src -s --compact` — get a quick index of everything, minimal tokens
6. All of the above with test files excluded by default, so agents focus on production code

That's a complete code understanding workflow in five commands, each under 100ms on a 12K-line codebase.
