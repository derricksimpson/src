# Journal — TypeScript Module Path Alias Resolution

## Summary

Shipped automatic resolution of TypeScript/JavaScript module path aliases in `src --graph`. This was the single biggest gap in graph accuracy for real-world TS/JS projects — most modern codebases use `@/` or `~/` prefixed imports via `tsconfig.json` paths or Vite's `resolve.alias`, and all of those were silently dropped from the dependency graph.

## What We Built

A new `src/alias.rs` module that:
- Scans the project root for `tsconfig.json` (with one level of `extends` inheritance) and Vite config files
- Parses `compilerOptions.baseUrl` + `compilerOptions.paths` from tsconfig using regex (no `serde_json`)
- Parses `resolve.alias` from Vite configs using regex heuristics (object and array forms)
- Exposes `load_aliases()` and `resolve_alias()` as the public API

Changes to `typescript.rs`:
- `extract_imports` now emits `"alias:<specifier>"` markers for non-relative, non-npm import paths
- `is_potential_alias` distinguishes `@/foo` (alias) from `@angular/core` (npm) from `react` (bare npm)
- All existing relative import resolution unchanged

Changes to `graph.rs`:
- `build_graph` and `process_file` accept `&[AliasMapping]` parameter
- Candidate resolution loop detects `"alias:"` prefixed entries and resolves them
- Deduplication handles alias + relative pointing to the same file

Changes to `main.rs`:
- `execute_graph` calls `alias::load_aliases(root)` once before the parallel graph scan

## Design Decision: Option A

Alias resolution is entirely outside the `LangImports` trait. The trait signature is unchanged, the 7 other language handlers are completely untouched. The implementation lives in `graph.rs` and `alias.rs`, only consuming the raw specifiers from the TS handler.

## What We Didn't Do

- Full JSON parsing (no `serde_json`). Regex approach handles standard tsconfig including JSONC.
- Deep `extends` chains. One level of inheritance only.
- Dynamic Vite config evaluation. Common static patterns only.
- Side-effect imports or dynamic imports (separate roadmap item).

## Files Changed

| File | Change |
|------|--------|
| `src/alias.rs` | New — alias loading, tsconfig/Vite parsing, resolution |
| `src/main.rs` | Added `mod alias`, updated `execute_graph` |
| `src/graph.rs` | Updated signatures and candidate loop |
| `src/lang/typescript.rs` | Added alias specifier emission |
| `tests/fixtures/alias_project/` | New test fixture |
| `tests/fixtures/vite_project/` | New test fixture |
| `tests/integration_test.rs` | New integration tests |
