# Requirements Document

## Introduction

We're building `src` — a blazing-fast, AOT-compiled, single-binary CLI tool for interrogating source code. Think of it as a developer's Swiss Army knife for quickly understanding project structure, finding files by pattern, and searching file contents — all returning clean, structured YAML output with line numbers intact. Built in C# on .NET 10 with native AOT compilation, `src` prioritizes speed and minimal memory footprint through memory-mapped file I/O and aggressive parallelization.

The tool serves three primary modes of operation:
1. **Directory hierarchy** — quickly show the skeletal outline of a project's folder structure
2. **File hierarchy with pattern matching** — list files and folders filtered by glob/filename patterns
3. **Content search** — search within files using string or regex patterns, returning matched chunks with surrounding context lines in structured YAML

## Requirements

### Requirement 1: CLI Entry Point and Argument Parsing

**User Story:** As a developer, I want a single `src` command with intuitive flags and options, so that I can quickly query any codebase from my terminal without learning a complex interface.

#### Acceptance Criteria

1. WHEN the user invokes `src` with no arguments THEN the system SHALL display a concise usage summary and available commands/options.
2. WHEN the user invokes `src help` or `src --help` THEN the system SHALL display detailed help text describing all commands, options, and usage examples.
3. WHEN the user provides an unrecognized option or malformed arguments THEN the system SHALL display a clear error message and suggest the correct usage.
4. WHEN the user provides the `--version` flag THEN the system SHALL display the current version of the tool.
5. WHEN the user provides a `--timeout <seconds>` option THEN the system SHALL enforce a maximum execution time and terminate gracefully if exceeded, returning a timeout error in the YAML output.
6. WHEN the binary is published THEN it SHALL be a single-file, native AOT-compiled executable with no external runtime dependencies.

### Requirement 2: Directory Hierarchy Mode

**User Story:** As a developer, I want to quickly see the folder structure of a project that contains source code files, so that I can understand the project layout at a glance.

#### Acceptance Criteria

1. WHEN the user invokes `src` with no file-pattern or search flags (or an explicit `--dirs` flag) THEN the system SHALL output a YAML-formatted hierarchy of all directories that contain at least one source code file (recursively).
2. WHEN a directory contains no source code files and no subdirectories with source code files THEN the system SHALL omit that directory from the output.
3. WHEN the user provides a `--root <path>` option THEN the system SHALL use that path as the starting directory; IF no root is provided THEN the system SHALL default to the current working directory.
4. WHEN the user provides an `--exclude <pattern>` option THEN the system SHALL exclude directories matching that pattern (e.g., `node_modules`, `bin`, `obj`, `.git`).
5. WHEN common non-source directories are encountered (e.g., `node_modules`, `.git`, `bin`, `obj`, `dist`, `.vs`) THEN the system SHALL exclude them by default without requiring explicit `--exclude` flags.

### Requirement 3: File Hierarchy with Pattern Matching

**User Story:** As a developer, I want to list all files and folders matching a filename pattern, so that I can quickly locate specific types of files across a project.

#### Acceptance Criteria

1. WHEN the user provides a `--r <glob-pattern>` option THEN the system SHALL return a YAML-formatted hierarchy of directories and files whose filenames match the given glob pattern.
2. WHEN the glob pattern matches no files THEN the system SHALL return an empty YAML result set with a message indicating no matches were found.
3. WHEN multiple glob patterns are provided (comma-separated or via multiple `--r` flags) THEN the system SHALL match files against any of the provided patterns (OR logic).
4. WHEN the user combines `--r` with `--exclude` THEN the system SHALL apply exclusion patterns to both directories and filenames.
5. WHEN enumerating files THEN the system SHALL traverse directories in parallel where possible to maximize throughput on SSDs.

### Requirement 4: Content Search with Context

**User Story:** As a developer, I want to search for strings or regex patterns inside source files and get back structured YAML with the matching lines and surrounding context, so that I can find code references fast without opening an IDE.

#### Acceptance Criteria

1. WHEN the user provides a `--f <pattern>` option THEN the system SHALL search within all matched files for lines containing that pattern (literal string match by default).
2. WHEN the `--f` pattern contains regex metacharacters or the user provides a `--regex` flag THEN the system SHALL treat the pattern as a regular expression.
3. WHEN the `--f` pattern contains a pipe character (`|`) THEN the system SHALL treat it as an OR separator, matching any of the delimited terms (e.g., `--f Payments|Payment` matches either).
4. WHEN the user provides a `--pad <n>` option THEN the system SHALL include `n` lines of context before and after each matching line in the output.
5. IF `--pad` is not provided THEN the system SHALL default to 0 lines of context (only matching lines).
6. WHEN multiple matches exist within the same file and their context windows overlap THEN the system SHALL merge them into a single contiguous chunk to avoid duplicate lines.
7. WHEN the system reads file contents for searching THEN it SHALL use memory-mapped file I/O and process multiple files in parallel to maximize performance.
8. WHEN the `--timeout <seconds>` option is active AND the search exceeds the allowed time THEN the system SHALL cancel remaining work, return partial results collected so far, and include a `timeout: true` field in the YAML output.

### Requirement 5: YAML Output Format

**User Story:** As a developer (or an LLM-based tool), I want all output in a consistent, structured YAML format with line numbers, so that I can parse and consume the results programmatically.

#### Acceptance Criteria

1. WHEN the system produces output THEN it SHALL format the output as valid YAML.
2. WHEN file contents are included in the output THEN each line SHALL be prefixed with its 1-based line number followed by a period and a space (e.g., `1.  import ...`).
3. WHEN the output includes file entries THEN each entry SHALL include at minimum: `path` (relative to root) and `contents` (the matched/returned lines as a YAML literal block scalar).
4. WHEN search mode is active THEN the output root SHALL be `files:` containing a list of matched file entries.
5. WHEN directory hierarchy mode is active THEN the output SHALL represent the tree structure using nested YAML mappings.
6. WHEN an error occurs during processing of a specific file THEN the system SHALL include an `error` field on that file's entry rather than failing the entire operation.
7. WHEN the system encounters a binary or unreadable file THEN it SHALL skip that file and optionally note it in the output.

### Requirement 6: Performance and Resource Management

**User Story:** As a developer working on large codebases, I want `src` to return results in seconds even on huge repositories, so that I can use it in my daily workflow and in CI/CD pipelines without friction.

#### Acceptance Criteria

1. WHEN scanning directories THEN the system SHALL use parallel directory enumeration to saturate available I/O bandwidth.
2. WHEN reading file contents for search THEN the system SHALL use memory-mapped files (`MemoryMappedFile`) to avoid unnecessary copies and reduce GC pressure.
3. WHEN processing files in parallel THEN the system SHALL throttle concurrency using a semaphore bounded to a sensible multiple of available CPU cores (e.g., 2x `Environment.ProcessorCount`).
4. WHEN the user provides `--timeout <seconds>` THEN the system SHALL use a `CancellationTokenSource` with the specified timeout and propagate cancellation cooperatively through all parallel work.
5. WHEN the tool is published with AOT THEN it SHALL start up in under 50ms on a modern machine and produce no JIT compilation overhead.
6. WHEN processing completes THEN the system SHALL report elapsed time in the YAML output via a `meta` section (e.g., `elapsed_ms: 142`).

### Requirement 7: Help and Discoverability

**User Story:** As a new user of `src`, I want clear and contextual help text, so that I can learn the tool's capabilities without reading external docs.

#### Acceptance Criteria

1. WHEN the user runs `src help` THEN the system SHALL display a summary of all available modes (directory, file listing, content search) with examples.
2. WHEN the user runs `src --help` THEN the system SHALL display the same help as `src help`.
3. WHEN the user provides an invalid combination of flags THEN the system SHALL display a specific error message explaining which flags conflict and how to resolve it.
4. WHEN help text is displayed THEN it SHALL include at least one concrete usage example per mode of operation.

### Requirement 8: Sensible Defaults and Ergonomics

**User Story:** As a developer, I want the tool to "just work" with minimal flags for common use cases, so that I can be productive immediately.

#### Acceptance Criteria

1. WHEN no `--root` is specified THEN the system SHALL default to the current working directory.
2. WHEN no `--exclude` is specified THEN the system SHALL apply a built-in exclusion list for common non-source directories (`node_modules`, `.git`, `bin`, `obj`, `dist`, `.vs`, `__pycache__`, `.idea`, `.vscode`).
3. WHEN the user specifies `--no-defaults` THEN the system SHALL disable all default exclusions.
4. WHEN no `--pad` is specified THEN the system SHALL default to 0 context lines.
5. WHEN no `--timeout` is specified THEN the system SHALL run without a timeout (but can be cancelled via Ctrl+C / SIGINT and still produce partial output).
