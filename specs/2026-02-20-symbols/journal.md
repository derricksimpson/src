# Journal — `--symbols`, `--count`, `--stats`

## Summary

We added three new modes to `src` that pushed it from a text-level grep tool into a structural code intelligence engine. Symbol extraction gives you the skeleton of any codebase, match counting gives you pattern heatmaps at a glance, and stats gives you the full project profile in milliseconds. Here's how it went.

---

## Phase 1: Models and Foundation (Tasks 1.1–1.4)

Started by laying down the data structures. Added `SymbolEntry` and `SymbolFile` for the symbol pipeline — each symbol carries its kind, name, line, optional visibility, optional parent (for methods inside classes/impls), and the full signature line. For counting, a simple `CountEntry` with path and count. For stats, a trio: `LangStats` (per-extension breakdown), `StatsTotals`, and `LargestFile`.

Extended `OutputEnvelope` with three new optional fields — `symbols`, `counts`, `stats` — and added `total_matches` to `MetaInfo`. Then swept through every existing envelope construction in `main.rs` adding the new `None` fields. Boring but necessary — the compiler caught every miss.

## Phase 2: CLI Parsing (Tasks 2.1–2.3)

Added three new flags to the arg parser: `--symbols`/`--s`, `--count`, and `--stats`/`--st`. All booleans. The mutual exclusion logic grew from a simple 3-way check to a proper matrix — `--f`, `--lines`, `--graph`, `--symbols`, `--stats` are all exclusive with each other, and `--count` gets special treatment (requires `--f`, and the combination counts as a single exclusive entry). Clean error messages when you mix them wrong.

Updated the help text with new entries in both the Modes and Options sections, plus three new examples showing real-world usage.

## Phase 3: The `LangSymbols` Trait (Tasks 3.1–3.5)

This was the architectural heart of the feature. Created a new `LangSymbols` trait in `lang/mod.rs`, parallel to the existing `LangImports`. Same pattern: `extensions()` to declare what file types you handle, `extract_symbols()` to do the work. Added `SymbolInfo` as the internal representation and a `SYMBOL_HANDLERS` static array with `get_symbol_handler()` dispatch.

Then implemented it for all three languages:

**Rust** — The trickiest part was tracking `impl` blocks to assign `parent` context to methods. Line-by-line scan, watching for `impl TypeName` to enter a parent context and tracking brace depth to know when we leave it. Detects `fn`, `struct`, `enum`, `trait`, `type`, `const`, `mod` — with visibility parsing for `pub` and `pub(crate)`.

**TypeScript/JavaScript** — More patterns to match: `function`, `class`, `interface`, `type`, `enum`, `const`, arrow functions (`const name = (`), and `export` variants of everything. Class method detection tracks brace depth after a `class` declaration, picking up `public`/`private`/`protected` method modifiers. `export` becomes the visibility marker.

**C#** — Namespace tracking, class/interface/struct/enum detection with the full C# visibility keyword set (`public`, `private`, `protected`, `internal`, `protected internal`). Method detection inside class bodies using the `ReturnType Name(` pattern. The visibility extraction was more nuanced than Rust or TS since C# has five visibility levels.

Registered all three handlers in `SYMBOL_HANDLERS`. Adding a future language is still just "implement trait, add to array."

## Phase 4: Symbol Orchestrator (Tasks 4.1–4.2)

Created `symbols.rs` following the exact same pattern as `graph.rs` — take file paths, iterate in parallel with rayon, detect language from extension, read file content (mmap for large, buffered for small, skip binaries), call the handler, collect results, sort by path.

Wired it into `main.rs` with `execute_symbols()` — same shape as every other execute function. File discovery via `scanner::find_files`, processing via `symbols::extract_symbols`, envelope construction, YAML output.

## Phase 5: Count Mode (Tasks 5.1–5.2)

The simplest of the three. Created `count.rs` with a `count_matches` function that's basically the search pipeline with chunk-building replaced by a counter. Same mmap/buffered read strategy, same `Matcher` for pattern matching, just `lines.filter(|l| matcher.is_match(l)).count()` per file. Returns per-file counts and a total.

Wired into `main.rs` as `execute_count()`. Special validation: if `--count` is set but `--f` isn't, reject immediately. The `MetaInfo` now carries `total_matches` so the consumer knows the aggregate without summing.

## Phase 6: Stats Mode (Tasks 6.1–6.2)

Created `stats.rs`. For each file: grab metadata for byte size, count lines by counting `\n` bytes in the file content (mmap for big files — just a single pass over the mapped memory, no string splitting needed). Aggregate by extension into a `HashMap`, sort by total lines descending. Compute totals. Collect all files sorted by bytes descending, take the top 10 for the `largest` section.

Wired as `execute_stats()`. Skips files it can't read, skips binaries for line counting (but still includes their byte size). Fast even on large repos because we never parse content — just count newlines.

## Phase 7: YAML Output (Tasks 7.1–7.4)

Added three new write functions to `yaml_output.rs`, each following the existing patterns:

- `write_symbols()` — nested YAML with `files:` → `symbols:` per file, conditional fields for visibility/parent
- `write_counts()` — flat `files:` list with `path` and `count`
- `write_stats()` — three sections: `languages:`, `totals:`, `largest:`

Updated `write_envelope()` to dispatch to these when the corresponding fields are present. Added `totalMatches` emission in `write_meta()`.

## Phase 8: Wiring and Dispatch (Tasks 8.1–9.1)

Updated every existing `OutputEnvelope` construction to include the three new `None` fields. Set the final dispatch order in `execute()`: lines > graph > symbols > stats > count > search > file listing > tree. Verified all exit codes and timeout handling work consistently.

---

## What We Shipped

Three new modes that make `src` a code intelligence tool, not just a grep tool:

- `src --symbols --r *.rs` — See every function, struct, enum, trait at a glance with line numbers and signatures
- `src --r *.ts --f "import" --count` — Instant pattern distribution without reading content
- `src --stats` — Full project profile in milliseconds

The `LangSymbols` trait keeps the door wide open for new languages. Same binary, same speed, same YAML output. Just more ways to understand code.
