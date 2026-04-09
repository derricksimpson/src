# Requirements Document

## Introduction

The `--graph` mode currently produces an incomplete dependency graph for TypeScript/JavaScript projects that use module path aliases. Imports like `@/components/Foo`, `@utils/bar`, or `~/lib/api` are silently dropped because `extract_imports` in `typescript.rs` only resolves paths starting with `./` or `../`. Every other non-relative specifier is assumed to be an npm package and ignored.

This is a significant gap. Most real-world TS/JS projects — anything scaffolded with Vite, Next.js, Remix, Nuxt, Angular, or a custom tsconfig — use `@`-prefixed or tilde-prefixed path aliases as their primary import style. The graph is missing the majority of edges in those codebases.

The fix: teach `src` to detect `tsconfig.json` path mappings and Vite-style `resolve.alias` entries, then use them during graph resolution to map aliased imports to real project files. This is done outside the `LangImports` trait (Option A — alias resolution lives in `graph.rs`), so no changes are needed to the 7 other language handlers.

## Requirements

### Requirement 1: Detect and Parse tsconfig.json Path Aliases

**User Story:** As a developer using `src --graph` on a TypeScript project, I want the tool to automatically read my `tsconfig.json` `paths` and `baseUrl` settings, so that aliased imports appear correctly in the dependency graph.

#### Acceptance Criteria

1. WHEN `--graph` is invoked AND a `tsconfig.json` exists in the project root THEN the system SHALL read `compilerOptions.baseUrl` and `compilerOptions.paths` from that file.
2. WHEN `tsconfig.json` contains `"paths": { "@/*": ["./src/*"] }` THEN the system SHALL interpret `@/` as a prefix that maps to the `src/` directory relative to the tsconfig location (adjusted by `baseUrl` if present).
3. WHEN `compilerOptions.baseUrl` is set (e.g. `"."` or `"./src"`) THEN the system SHALL resolve all `paths` targets relative to the `baseUrl` directory.
4. WHEN `compilerOptions.baseUrl` is absent THEN the system SHALL resolve `paths` targets relative to the directory containing the `tsconfig.json`.
5. WHEN a `paths` entry has multiple targets (e.g. `"@/*": ["./src/*", "./generated/*"]`) THEN the system SHALL generate candidates from all targets during resolution.
6. WHEN `tsconfig.json` contains an `"extends"` field referencing another config (e.g. `"extends": "./tsconfig.base.json"`) THEN the system SHALL follow one level of extends to inherit `baseUrl` and `paths` from the parent config, with the child's values taking precedence.
7. WHEN no `tsconfig.json` is found in the project root THEN the system SHALL proceed without alias resolution (current behavior, no error).
8. WHEN `tsconfig.json` exists but contains no `compilerOptions.paths` THEN the system SHALL proceed without alias resolution (no error, no warning).
9. WHEN `tsconfig.json` is malformed or unreadable THEN the system SHALL emit a non-fatal warning and proceed without alias resolution.

### Requirement 2: Detect and Parse Vite Config Aliases

**User Story:** As a developer using Vite with custom `resolve.alias` config, I want `src --graph` to pick up my Vite aliases, so that Vite-style imports are resolved in the graph.

#### Acceptance Criteria

1. WHEN `--graph` is invoked AND a `vite.config.ts`, `vite.config.js`, `vite.config.mts`, or `vite.config.mjs` exists in the project root THEN the system SHALL attempt to extract `resolve.alias` mappings from it.
2. WHEN the Vite config contains an alias in object form (e.g. `'@': path.resolve(__dirname, 'src')` or `'@': '/src'` or `'@': './src'`) THEN the system SHALL extract the alias prefix and target directory.
3. WHEN the Vite config contains an alias in array form (e.g. `[{ find: '@', replacement: '/src' }]`) THEN the system SHALL extract the alias prefix and target directory.
4. WHEN the Vite config uses dynamic expressions that cannot be statically analyzed THEN the system SHALL skip those entries silently (best-effort extraction).
5. WHEN both a `tsconfig.json` and a Vite config define aliases THEN tsconfig aliases SHALL take precedence (tsconfig is the canonical source of truth for TS; Vite aliases are fallback/supplementary).
6. WHEN no Vite config file is found THEN the system SHALL proceed without Vite alias resolution (no error).

### Requirement 3: Resolve Aliased Imports in the Dependency Graph

**User Story:** As a user running `src --graph`, I want imports like `@/components/Button` to appear as resolved edges in the graph output, so that the dependency map is complete.

#### Acceptance Criteria

1. WHEN a TS/JS file contains `import { X } from '@/components/Button'` AND `@/` maps to `src/` THEN the system SHALL generate candidate paths and resolve against the project file set.
2. WHEN an aliased import resolves to a project file THEN that file SHALL appear in the `imports` array for the importing file's graph entry.
3. WHEN an aliased import does not resolve to any project file THEN it SHALL be silently dropped.
4. WHEN the same file is imported via both a relative path and an alias THEN it SHALL appear only once in the `imports` array (deduplication).
5. WHEN alias resolution is active THEN the existing behavior for relative imports (`./`, `../`) and npm package imports SHALL remain unchanged.
6. WHEN `export { X } from '@/utils/foo'` appears (re-export with alias) THEN the system SHALL resolve it identically to an `import` statement.
7. WHEN `const x = require('@/utils/foo')` appears (CommonJS with alias) THEN the system SHALL resolve it identically to an ES import.

### Requirement 4: TypeScript Handler Emits Raw Non-Relative Import Specifiers

**User Story:** As the graph resolution pipeline, I need the TypeScript import handler to return raw import specifiers for non-relative, non-npm imports, so that alias resolution can process them.

#### Acceptance Criteria

1. WHEN `extract_imports` encounters an import path that does not start with `./` or `../` THEN it SHALL return the raw specifier string prefixed with `alias:` to distinguish it from resolved relative paths.
2. WHEN an import path starts with `@` followed by `/` (e.g. `@/foo`) THEN the system SHALL treat it as a potential alias.
3. WHEN an import path starts with `@` followed by a lowercase word and `/` (e.g. `@angular/core`) THEN the system SHALL treat it as an npm scoped package and skip it.
4. WHEN an import path starts with `~/` THEN the system SHALL treat it as a potential alias.
5. WHEN an import path is a bare identifier (e.g. `react`, `express`) THEN the system SHALL skip it.

### Requirement 5: No Changes to Other Language Handlers

**User Story:** As the maintainer of `src`, I want alias resolution implemented without modifying the `LangImports` trait signature or other language handlers.

#### Acceptance Criteria

1. The `LangImports` trait signature SHALL remain unchanged.
2. The files `rust.rs`, `csharp.rs`, `go.rs`, `java.rs`, `kotlin.rs`, `ruby.rs`, `python.rs` SHALL not be modified.
3. Alias resolution SHALL be performed in `graph.rs` and `alias.rs`, outside the trait.

### Requirement 6: No New Heavy Dependencies

**User Story:** As the maintainer of `src`, I want to avoid adding large dependencies like `serde_json`.

#### Acceptance Criteria

1. Parsing SHALL use the existing `regex` crate and hand-rolled string extraction.
2. No new crate dependencies SHALL be added.
