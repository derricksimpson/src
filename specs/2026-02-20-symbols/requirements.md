# Requirements Document

## Introduction

We're adding three new modes to `src` that push it from a text-level code interrogation tool into a structural code intelligence engine. These features give developers and LLM agents the ability to understand code at the symbol level, get quick match statistics, and get an instant statistical profile of any codebase — all without leaving the same fast, YAML-output, single-binary workflow that `src` already provides.

The three features are:

1. **`--symbols` / `--s`** — Language-aware symbol extraction. Scans source files and emits function, struct, class, enum, trait, interface, and type declarations with their kind, name, visibility, line location, and signature. Uses the existing pluggable `lang/` trait system, extended with a new `LangSymbols` trait so each language handler can define its own extraction logic. Ships with support for Rust, TypeScript/JavaScript, and C# — extensible to any future language by implementing a single trait.

2. **`--count`** — Match count mode. A companion to `--f` that returns per-file match counts instead of full content chunks. Same search pipeline, dramatically smaller output. Ideal for "where is this pattern hot?" questions.

3. **`--stats` / `--st`** — Codebase statistics. Scans the project and emits a breakdown by file extension: file count, total lines, total bytes, plus overall totals and a list of the largest files. No content reading beyond metadata and line counting — fast even on massive repos.

All three integrate with existing flags (`--r`, `--root`/`-d`, `--timeout`, `--exclude`, `--no-defaults`) and emit structured YAML through the existing `OutputEnvelope` + `yaml_output` pipeline.

## Requirements

### Requirement 1: Symbol Extraction Mode (`--symbols` / `--s`)

**User Story:** As a developer or LLM agent, I want to extract function, class, struct, enum, trait, and type declarations from source files with their names, kinds, visibility, line locations, and signatures, so that I can understand code structure without reading full file contents.

#### Acceptance Criteria

1. WHEN the user provides the `--symbols` or `--s` flag THEN the system SHALL scan all matched files and emit a YAML structure listing each file's symbols.
2. WHEN `--symbols` is combined with `--r <glob>` THEN the system SHALL scope symbol extraction to files matching the glob patterns.
3. WHEN `--symbols` is provided without `--r` THEN the system SHALL scan all non-excluded source files (same behavior as `--f` without `--r`).
4. WHEN a file is scanned for symbols THEN for each declaration the system SHALL emit: `kind` (fn, struct, class, enum, trait, interface, type, method, const), `name`, `line` (1-based start line), and `signature` (the declaration line or first line of the declaration).
5. WHEN a symbol has a visibility modifier (pub, export, public, etc.) THEN the system SHALL include a `visibility` field on that symbol entry.
6. WHEN a symbol is a method inside a class or impl block THEN the system SHALL include a `parent` field referencing the containing type name.
7. WHEN `--symbols` is combined with `--f` or `--lines` or `--graph` or `--count` or `--stats` THEN the system SHALL reject the combination with a clear mutual exclusion error.
8. WHEN the Rust language handler extracts symbols THEN it SHALL recognize: `fn`, `pub fn`, `struct`, `pub struct`, `enum`, `pub enum`, `trait`, `pub trait`, `type`, `pub type`, `const`, `pub const`, `impl` blocks (to establish parent context for methods), and `mod` declarations.
9. WHEN the TypeScript/JavaScript language handler extracts symbols THEN it SHALL recognize: `function`, `export function`, `export default function`, `class`, `export class`, `interface`, `export interface`, `type`, `export type`, `const`, `export const`, `enum`, `export enum`, arrow function assignments (`const name = (...) =>`), and `export default`.
10. WHEN the C# language handler extracts symbols THEN it SHALL recognize: `class`, `public class`, `interface`, `public interface`, `struct`, `public struct`, `enum`, `public enum`, method declarations within classes, `namespace` declarations, and property declarations.
11. WHEN a file has no extension or an unsupported extension THEN the system SHALL skip that file (no entry emitted) during symbol extraction.
12. WHEN `--line-numbers off` is provided with `--symbols` THEN it SHALL have no effect (symbols always include line numbers as structural metadata, not as content prefixes).
13. WHEN `--timeout` is active and the symbol extraction exceeds the allowed time THEN the system SHALL cancel remaining work and return partial results with `meta.timeout: true`.
14. WHEN the system encounters an error reading a specific file during symbol extraction THEN it SHALL include an `error` field on that file's entry and continue with remaining files.
15. WHEN adding a new language handler for symbols in the future THEN the developer SHALL only need to implement a single `LangSymbols` trait method and register the handler — no other files need to change.

### Requirement 2: Match Count Mode (`--count`)

**User Story:** As a developer or LLM agent, I want to get per-file match counts for a search pattern without the full content output, so that I can quickly understand pattern distribution across a codebase with minimal output volume.

#### Acceptance Criteria

1. WHEN the user provides both `--f <pattern>` and `--count` THEN the system SHALL return per-file match counts instead of content chunks.
2. WHEN `--count` is used THEN each file entry SHALL include only `path` and `count` (number of matching lines) — no `contents` or `chunks`.
3. WHEN `--count` is used THEN the `meta` section SHALL include a `totalMatches` field with the sum of all per-file counts.
4. WHEN `--count` is provided without `--f` THEN the system SHALL reject with a clear error: `"--count requires --f <pattern>"`.
5. WHEN `--count` is combined with `--symbols`, `--lines`, `--graph`, or `--stats` THEN the system SHALL reject with a mutual exclusion error.
6. WHEN `--count` is used with `--r <glob>` THEN the system SHALL scope the search to files matching the glob, same as normal `--f` behavior.
7. WHEN `--count` is used THEN `--pad` SHALL be ignored (no context lines needed for counting).
8. WHEN `--count` is used THEN `--line-numbers` SHALL have no effect (no content is emitted).
9. WHEN `--timeout` is active during `--count` THEN the system SHALL return partial results with `meta.timeout: true`.
10. WHEN no files match the pattern THEN the system SHALL return an empty `files: []` with `meta.totalMatches: 0`.
11. WHEN files match THEN the output SHALL be sorted by path (ascending, case-insensitive) consistent with existing output ordering.

### Requirement 3: Codebase Statistics Mode (`--stats` / `--st`)

**User Story:** As a developer or LLM agent, I want to get an instant statistical profile of a codebase — file counts, line counts, and byte sizes broken down by language/extension — so that I can understand project scope and composition at a glance.

#### Acceptance Criteria

1. WHEN the user provides `--stats` or `--st` THEN the system SHALL scan all non-excluded source files and emit a statistical breakdown.
2. WHEN `--stats` is combined with `--r <glob>` THEN the system SHALL scope statistics to files matching the glob patterns.
3. WHEN statistics are computed THEN the output SHALL include a `languages` section grouped by file extension, where each entry contains: `extension`, `files` (count), `lines` (total), and `bytes` (total).
4. WHEN statistics are computed THEN the output SHALL include a `totals` section with: `files`, `lines`, and `bytes` aggregated across all extensions.
5. WHEN statistics are computed THEN the output SHALL include a `largest` section listing the top 10 files by byte size, each with `path`, `lines`, and `bytes`.
6. WHEN `--stats` is combined with `--f`, `--lines`, `--graph`, `--symbols`, or `--count` THEN the system SHALL reject with a mutual exclusion error.
7. WHEN `--timeout` is active during `--stats` THEN the system SHALL return partial results with `meta.timeout: true`.
8. WHEN a file cannot be read (permissions, etc.) THEN the system SHALL skip it and not include it in statistics (no error entries for stats mode).
9. WHEN the `languages` section is emitted THEN entries SHALL be sorted by total lines descending (largest language first).
10. WHEN the `largest` section is emitted THEN entries SHALL be sorted by bytes descending.
11. WHEN `--stats` is used THEN `--pad`, `--line-numbers`, and `--regex` SHALL have no effect.
12. WHEN `--stats` is provided without `--r` THEN the system SHALL scan all non-excluded source files (using the same `SOURCE_EXTENSIONS` list from `scanner.rs`).

### Requirement 4: CLI Integration and Mutual Exclusion Updates

**User Story:** As a user of `src`, I want clear and consistent behavior when combining new flags with existing ones, so that I never get confusing or silent failures.

#### Acceptance Criteria

1. WHEN the updated mutual exclusion rules are enforced THEN the following flags SHALL be mutually exclusive with each other: `--f` (without `--count`), `--f --count`, `--lines`, `--graph`, `--symbols`, `--stats`.
2. WHEN the user provides an invalid combination THEN the error message SHALL list the conflicting flags by name.
3. WHEN `--help` is displayed THEN it SHALL include entries for `--symbols`/`--s`, `--count`, and `--stats`/`--st` with descriptions and examples.
4. WHEN the dispatch matrix is evaluated THEN the priority SHALL be: `--lines` > `--graph` > `--symbols` > `--stats` > `--f --count` > `--f` > `--r` only > default tree.
