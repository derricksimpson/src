# Requirements: `src` CLI Tool

Rewriting from scratch in rust.

## Overview

**`src`** is a blazing-fast CLI tool for interrogating source code. It provides three operational modes:

1. **Directory hierarchy** — Explore and navigate project structure
2. **File listing with pattern matching** — Find files by glob patterns
3. **Content search** — Search within file contents with context

This document defines the functional and non-functional requirements for the `src` CLI.

---

## 1. CLI Parsing

- **REQ-CLI-001** The tool shall accept a subcommand-based interface with `src <command> [options] [args]`.
- **REQ-CLI-002** Subcommands shall include: `tree`, `list` (or `ls`), and `search` (or `grep`).
- **REQ-CLI-003** The parser shall support standard long-form options (e.g., `--output`, `--include`) and short aliases (e.g., `-o`, `-i`).
- **REQ-CLI-004** Invalid or unknown options shall produce a clear error message and non-zero exit.
- **REQ-CLI-005** The parser shall be resilient to argument order where semantically valid.

---

## 2. Directory Hierarchy

- **REQ-TREE-001** The `tree` command shall output a hierarchical representation of directories and files under a given path.
- **REQ-TREE-002** A default root path of `.` (current directory) shall be used when none is specified.
- **REQ-TREE-003** The hierarchy shall respect configurable depth limits.
- **REQ-TREE-004** Directories and files shall be included/excluded via configurable filters (e.g., `.gitignore`, explicit exclusions).
- **REQ-TREE-005** Output shall be emitted in YAML format for machine parsing and human readability.

---

## 3. File Listing with Pattern Matching

- **REQ-LIST-001** The `list` command shall list files matching one or more glob patterns.
- **REQ-LIST-002** Glob patterns shall support common wildcards (e.g., `*.ts`, `**/*.test.ts`).
- **REQ-LIST-003** Search shall be recursive by default with an optional maximum depth.
- **REQ-LIST-004** Exclusions (e.g., `node_modules`, `.git`) shall apply unless explicitly overridden.
- **REQ-LIST-005** Results shall include file path, optionally size and metadata.

---

## 4. Content Search with Context

- **REQ-SEARCH-001** The `search` command shall search file contents using literal or regex patterns.
- **REQ-SEARCH-002** Matches shall include configurable context lines (before/after).
- **REQ-SEARCH-003** Search shall be constrained by file type/extension where specified.
- **REQ-SEARCH-004** Line numbers shall be included for each match.
- **REQ-SEARCH-005** Exclusions shall apply to directories and files (e.g., binaries, vendored code).
- **REQ-SEARCH-006** Case-sensitive and case-insensitive modes shall be supported.

---

## 5. YAML Output Format

- **REQ-OUT-001** All commands shall support a `--output yaml` (or default) format.
- **REQ-OUT-002** YAML output shall be well-formed and parseable by standard YAML processors.
- **REQ-OUT-003** Output structure shall be consistent per command with clear keys (e.g., `files`, `matches`, `tree`).
- **REQ-OUT-004** Optional human-friendly formats (e.g., table, plain text) may be supported as alternatives.

---

## 6. Performance Management

- **REQ-PERF-001** The tool shall complete directory scans on typical codebases (< 10k files) in under two seconds.
- **REQ-PERF-002** A configurable timeout shall cap long-running operations (e.g., search over large trees).
- **REQ-PERF-003** Content search shall use streaming or chunked processing where practicable to minimize memory.
- **REQ-PERF-004** Concurrency (e.g., parallel file reads) shall be bounded to avoid resource exhaustion.
- **REQ-PERF-005** Large output shall be streamed to stdout rather than buffered entirely in memory.

---

## 7. Help and Discoverability

- **REQ-HELP-001** `src --help` and `src -h` shall display a brief overview and list of commands.
- **REQ-HELP-002** `src <command> --help` shall display usage, options, and examples for that command.
- **REQ-HELP-003** Help text shall include examples for common use cases.
- **REQ-HELP-004** Invalid usage (e.g., missing required args) shall suggest correct usage or link to help.

---

## 8. Sensible Defaults

- **REQ-DEF-001** Default root path shall be the current working directory.
- **REQ-DEF-002** Default exclusions shall include: `node_modules`, `.git`, `__pycache__`, `.venv`, `dist`, `build`, and common build artifacts.
- **REQ-DEF-003** Default output format shall be YAML.
- **REQ-DEF-004** Default search context shall be one line before and after a match.
- **REQ-DEF-005** Default timeout shall be sufficient for typical usage (e.g., 30 seconds) with override via option.
- **REQ-DEF-006** Case-sensitive search shall be the default; case-insensitive mode via `-i` or `--ignore-case`.
