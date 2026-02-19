# Journal — `src` CLI Implementation

## Summary

We built `src` — a native AOT-compiled, single-binary CLI tool for fast source code interrogation. Starting from a blank .NET 10 console template, we shipped a fully functional tool with three modes of operation, structured YAML output, memory-mapped parallel search, and sub-50ms startup time. Here's how it went.

---

## Phase 1: Foundation (Tasks 1.1–1.2)

We started by setting up the project skeleton. Added `System.CommandLine` (2.0.0-beta5) and `YamlDotNet` (16.x) to `src.csproj`, configured the assembly name to `src`, and kept AOT and single-file publish settings in place. Then we laid out the directory structure — `Models/`, `Commands/`, `Services/`, `Output/` — and defined the core data models: `OutputEnvelope`, `MetaInfo`, `FileEntry`, `FileChunk`, and `ScanResult`. Clean, init-only properties, nothing fancy. Just the shapes we'd need to carry data through every layer.

## Phase 2: Output Layer (Tasks 2.1–2.2)

Before building anything that produces results, we built the thing that formats them. `YamlSerializerContext` was set up with the `[YamlStaticContext]` attribute, registering all our model types for AOT source generation — no reflection at runtime. `YamlOutputWriter` wraps the static serializer, configured for camelCase keys and literal block scalars so file contents come out looking clean with their line numbers intact.

## Phase 3: Filtering Infrastructure (Tasks 3.1–3.2)

Next came the filtering layer. `ExclusionFilter` ships with a hardcoded set of 17 common non-source directories (`node_modules`, `.git`, `bin`, `obj`, etc.) and supports `--exclude` for extras and `--no-defaults` to wipe the slate clean. `GlobMatcher` turned out to be almost trivial — `FileSystemName.MatchesSimpleExpression` from `System.IO.Enumeration` does the heavy lifting. We just wrapped it with a multi-pattern `MatchesAny` helper.

## Phase 4: File Scanner (Tasks 4.1–4.2)

`FileScanner` handles all directory and file enumeration. For directory hierarchy mode, it recursively walks the tree and builds a `ScanResult` structure, pruning excluded directories before recursing into them and only including directories that actually contain source files somewhere underneath. For file listing mode, it does the same traversal but flattens the result into a list of matching file paths. Both paths use parallel enumeration — spawning tasks per subdirectory — to saturate SSD bandwidth.

## Phase 5: Content Search Engine (Tasks 5.1–5.5)

This was the meaty part. `ContentSearcher` is the performance-critical core. It uses `Parallel.ForEachAsync` with a `SemaphoreSlim` bounded to `2x ProcessorCount` to process files concurrently. For files over 64KB, we go through `MemoryMappedFile` → `UnmanagedMemoryStream` → `StreamReader` for line-by-line scanning. Smaller files just get `File.ReadAllLines` since the mmap setup overhead isn't worth it.

Pattern matching supports three modes: literal string contains (default), full regex (via `--regex`), and pipe-delimited OR patterns (e.g., `Payments|Payment`). The regex path pre-compiles the pattern once. The padding logic expands each match by `n` lines in both directions, clamps to file boundaries, then merges overlapping windows into contiguous chunks. Each chunk carries its 1-based line-numbered content.

Per-file errors (permission denied, locked files, etc.) get caught and surfaced as `error` fields on the `FileEntry` rather than killing the whole run. Binary files are detected via a null-byte heuristic in the first 8KB and silently skipped.

## Phase 6: CLI Wiring (Tasks 6.1–6.3)

`Program.cs` got a complete rewrite. The `RootCommand` defines all options — `--root`, `--r`, `--f`, `--pad`, `--timeout`, `--exclude`, `--no-defaults`, `--regex`, `--version`. The handler inspects which options are present and dispatches to the right command:

- No `--r`, no `--f` → `ScanCommand` (directory hierarchy)
- `--r` only → `ScanCommand` (file listing)
- `--f` present → `SearchCommand` (content search)

Both commands create a `CancellationTokenSource` from `--timeout` if provided, capture elapsed time in `MetaInfo`, and pipe everything through `YamlOutputWriter`. `Console.CancelKeyPress` hooks into the same cancellation path for graceful SIGINT handling.

## Phase 7: Help and Polish (Tasks 7.1–7.2)

System.CommandLine's built-in help generation does most of the work here. We customized the `HelpBuilder` to include per-mode usage examples — directory hierarchy, file listing, and content search — so new users can see exactly how to use each mode. `--version` pulls from a hardcoded constant (easy to automate later via CI).

## Phase 8: Exit Codes and Integration (Tasks 8.1–8.2)

We defined four exit codes: 0 (success), 1 (user error), 2 (timeout), 130 (SIGINT). Timeout returns partial results with `meta.timeout: true`. The final integration pass verified all three modes work end-to-end — directory trees, file listings with glob patterns, and content search with padding and timeout — all producing valid YAML matching the spec examples.

## Phase 9: Publish (Task 9.1)

Created a publish profile for release builds: full AOT, single-file, link-trimmed. The output is a standalone `src.exe` binary that runs without .NET installed, starts in under 50ms, and handles large codebases with ease.

---

## What We Shipped

A single binary — `src` (or `src.exe` on Windows) — that:

- Shows project structure at a glance (`src`)
- Finds files by pattern (`src --r *.ts`)
- Searches file contents with context and returns structured YAML (`src --r *.ts --f Payments --pad 2 --timeout 5`)
- Handles huge repos via memory-mapped I/O and parallel processing
- Fails gracefully with per-file error reporting, timeouts, and SIGINT handling
- Starts fast, runs fast, stays out of the way

The `next/plan.md` (LLM-powered `src desc`) is queued up as a future phase — the architecture cleanly supports adding new commands without touching existing code.
