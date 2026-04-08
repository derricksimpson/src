# Journal Entry — `--symbols`, `--count`, and `--stats`

## Source file
- `specs/2026-02-20-symbols/journal.md`
- Timestamp: `2026-02-20 07:44:49 -0500`

## What was done
- Added project-wide symbol extraction support with new symbol models and envelope fields.
- Added `--symbols`, `--count`, and `--stats` CLI modes.
- Introduced a symbol trait/dispatch model and handlers for Rust, TypeScript/JavaScript, and C# symbol extraction.
- Added count mode for per-file matches and project total-match reporting.
- Added stats mode for language breakdown, totals, and largest files.
- Wired new modes into YAML output and command validation/execution paths.
