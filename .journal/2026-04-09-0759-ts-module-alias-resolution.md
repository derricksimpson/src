# Journal Entry — TypeScript Module Alias Resolution

## Source file
- `specs/2026-04-08-ts-module-aliases/journal.md`
- Timestamp: `2026-04-09 07:59:04 -0400`

## What was done
- Added alias resolution support for TypeScript/JavaScript module path aliases.
- Implemented alias parsing from `tsconfig.json` (`paths` + `baseUrl`) and Vite config aliases.
- Tagged non-relative specifiers and resolved them during dependency graph construction.
- Kept `LangImports` API unchanged by applying alias mapping in the graph layer.
- Added fixtures and integration coverage for alias-heavy projects.
