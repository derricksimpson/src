# Implementation Plan

#[[file:requirements.md]]
#[[file:design.md]]

- [x] 1. Create the `alias.rs` module with data types and tsconfig parser
  - [x] 1.1 Create `src/alias.rs` with `AliasMapping` struct, `load_aliases`, `resolve_alias`, `is_potential_alias`, `is_npm_scoped_package`
    - _Requirements: 1.1, 5.3, 6.1_
  - [x] 1.2 Implement `parse_tsconfig` — locate and read `tsconfig.json`, extract `baseUrl` and `paths` using regex
    - _Requirements: 1.1-1.4, 1.7-1.9_
  - [x] 1.3 Implement `extends` handling — one level of tsconfig inheritance
    - _Requirements: 1.6_
  - [x] 1.4 Implement JSONC comment stripping for tsconfig files with `//` and `/* */` comments
    - _Requirements: 1.9_
  - [x] 1.5 Write unit tests for tsconfig parsing, alias resolution, npm scope detection
    - _Requirements: 1.1-1.9_

- [x] 2. Add Vite config parser to `alias.rs`
  - [x] 2.1 Implement `parse_vite_config` — locate Vite config files
    - _Requirements: 2.1, 2.6_
  - [x] 2.2 Implement regex extraction for Vite object-form and array-form aliases
    - _Requirements: 2.2-2.4, 6.2_
  - [x] 2.3 Implement alias precedence merging in `load_aliases` (tsconfig wins)
    - _Requirements: 2.5_
  - [x] 2.4 Write unit tests for Vite config parsing
    - _Requirements: 2.1-2.6_

- [x] 3. Modify the TypeScript handler to emit raw alias specifiers
  - [x] 3.1 Add `use crate::alias` import to `typescript.rs`
  - [x] 3.2 Update `extract_imports` to emit `"alias:{path}"` for non-relative non-npm imports
    - _Requirements: 4.1-4.5, 3.6, 3.7_
  - [x] 3.3 Write unit tests for alias emission, npm filtering, relative preservation
    - _Requirements: 4.1-4.5_

- [x] 4. Wire alias resolution into `graph.rs` and `main.rs`
  - [x] 4.1 Update `build_graph` and `process_file` signatures to accept `&[AliasMapping]`
    - _Requirements: 5.3_
  - [x] 4.2 Update `process_file` candidate loop to handle `"alias:"` prefixed entries
    - _Requirements: 3.1-3.5_
  - [x] 4.3 Register `mod alias` in `main.rs`, call `load_aliases` in `execute_graph`
    - _Requirements: 1.1, 2.1_

- [x] 5. Add integration test fixtures and integration tests
  - [x] 5.1 Create `tests/fixtures/alias_project/` with tsconfig, TS files using `@/` and `~/` aliases
    - _Requirements: 1.1-1.5, 3.1_
  - [x] 5.2 Create `tests/fixtures/vite_project/` with vite.config.ts and TS files using `@/` alias
    - _Requirements: 2.1-2.3_
  - [x] 5.3 Write integration tests verifying graph output resolves aliases correctly
    - _Requirements: 3.1-3.7_
