# Overview

This repository builds `src`, a fast Rust CLI for interrogating source code in
parallel. The crate name is `src-cli`; the shipped binary is `src` from
`src/main.rs`.

The tool is optimized for agent and engineering workflows that need structured
answers instead of separate `find`, `grep`, and file-read commands. YAML is the
default output format, with JSON available through `--json` or
`--format json`.

Core modules:

- `src/main.rs` owns process entry, mode dispatch, output envelopes, limits,
  timeout/cancel handling, and output-file routing.
- `src/cli.rs` owns argument parsing, mode exclusivity, help text, aliases, and
  option validation.
- `src/models.rs` owns shared output payload structs used by YAML and JSON
  writers.
- `src/scanner.rs` discovers source files in parallel and applies default,
  custom, and test-file exclusions.
- `src/searcher.rs`, `src/lines.rs`, `src/graph.rs`, `src/symbols.rs`,
  `src/callers.rs`, `src/count.rs`, and `src/stats.rs` implement the CLI modes.
- `src/lang/` contains per-language import and symbol extraction handlers.
  Register new language handlers in `src/lang/mod.rs`.
- `src/yaml_output.rs` writes both YAML and JSON output. Keep both formats in
  sync when changing payloads.
- `src/alias.rs` supports TypeScript/Vite alias resolution for graph mode.
- `tests/integration_test.rs` is the main behavioral contract for CLI flags,
  output shape, test-file filtering, aliases, callers, comments, JSON, and
  error handling.

# Coding Guidelines

- Keep changes small and mode-oriented. Extend the existing mode module when
  possible instead of adding parallel logic in `main.rs`.
- Preserve the `main.rs` flow: parse CLI args, find candidate files, execute one
  mutually exclusive mode, emit an `OutputEnvelope`, and return an exit code.
- When adding or changing flags, update `CliArgs`, `parse_args`, validation
  rules, `print_help`, README examples when appropriate, and integration tests.
- Keep output structs in `models.rs` as the shared contract. Update YAML and
  JSON writers together for any payload or metadata change.
- Preserve deterministic output ordering. Most parallel paths sort results
  case-insensitively before returning.
- Preserve test-file filtering semantics: tests are excluded by default for
  scan/search/symbol/stats/count-style flows and included only with
  `--with-tests`; explicit `--lines` paths can still read test files.
- Use `rayon` for parallel file work and keep cancellation checks near parallel
  loops.
- Use `file_reader::read_file` for source reads unless a module has a specific
  performance reason, as `stats.rs` does for fast line counts.
- Normalize emitted paths through `path_helper::normalized_relative` so Windows
  paths remain stable and use forward slashes in output.
- Keep parsers conservative. Language handlers are lightweight scanners, not
  full AST parsers; prefer predictable heuristics plus fixtures/tests.
- Avoid adding dependencies unless they clearly simplify a real feature or
  materially improve correctness/performance.
- Keep comments sparse and useful. The existing code favors readable functions
  over explanatory comments.

Validation:

- Run `cargo test` for normal local verification.
- Run `cargo build` when touching binary startup, platform-specific behavior, or
  dependency configuration.
- CI runs `cargo build` and `cargo test --verbose` on Ubuntu for pushes and PRs
  targeting `main` or `dev`.
- For output-format changes, add or update integration tests for both YAML and
  JSON where the behavior is visible.
- For language parser changes, update fixtures under `tests/fixtures/` and add
  focused integration coverage.

# Tools

## journal

The journal skill must be understood and used along with the target folder:  ./.journal

## src

IMPORTANT: use the `src` skill  itself as the primary repo exploration tool in this
repository. Prefer one batched `src` invocation over many separate file reads,
greps, or directory walks.

Start with repo shape:

```bash
src
src --stats
```

Map Rust dependencies before changing shared modules:

```bash
src --graph -g "*.rs"
```

Scan declarations before opening implementations:

```bash
src --symbols -g "*.rs" --compact
```

Batch exact file reads into one command:

```bash
src --lines "src/main.rs:157:203 src/cli.rs:40:219 src/models.rs:95:116"
```

Use context windows for searches that would otherwise dump full files:

```bash
src -f "OutputEnvelope|MetaInfo" -g "*.rs" -C 3
```

Trace declarations and call sites together:

```bash
src --callers process_file -g "*.rs"
```

Include tests explicitly when you need test coverage or fixture behavior:

```bash
src --symbols -d tests -g "*.rs" --with-tests --compact
src --lines "tests/integration_test.rs:1:120 tests/integration_test.rs:880:1051"
```

Useful commands:

```bash
cargo test
cargo build
cargo test --verbose
```
