# Requirements Document

## Introduction

We're adding Go and Python language handlers to `src`'s pluggable `lang/` system. These two languages round out the top 5 most-used languages alongside the existing Rust, TypeScript/JavaScript, and C# handlers. Each handler implements the `LangImports` trait (for `--graph` dependency graph support) and, if the symbols spec has been completed, the `LangSymbols` trait (for `--symbols` symbol extraction support).

Go and Python have fundamentally different import systems from the three languages we already support:

- **Go** uses package-path-based imports (`import "project/internal/auth"`) where the path corresponds to a directory, not a specific file. Relative imports don't exist — everything is an absolute package path relative to the module root defined in `go.mod`.
- **Python** uses both absolute and relative imports (`import foo.bar`, `from . import baz`, `from ..utils import helper`). Relative imports use dot notation. Module resolution maps to both files (`foo/bar.py`) and packages (`foo/bar/__init__.py`).

Both require reading a project manifest to understand the module root: `go.mod` for Go, and the project root itself (or `pyproject.toml`/`setup.py`) for Python.

The handlers follow the exact same pattern as existing ones — implement the trait, register in the static array, done. No other files need to change for `--graph` support. The dependency graph, scanner, and YAML output layers are all language-agnostic.

## Requirements

### Requirement 1: Go Language Handler — Import Extraction (`--graph`)

**User Story:** As a developer working with Go codebases, I want `src --graph` to show which Go files depend on which other Go files within my project, so that I can understand internal package dependencies without external tooling.

#### Acceptance Criteria

1. WHEN a `.go` file is processed for `--graph` THEN the handler SHALL recognize `import "path/to/package"` (single import) and `import ( "path1" \n "path2" )` (grouped import) statements.
2. WHEN an import path matches the module path prefix from `go.mod` THEN the handler SHALL treat it as a project-internal import and resolve it to a directory path within the project.
3. WHEN an import path does not match the module path prefix (e.g., `fmt`, `net/http`, `github.com/other/pkg`) THEN the handler SHALL exclude it as an external dependency.
4. WHEN resolving a package import to files THEN the handler SHALL map the package path to the corresponding directory and include all `.go` files in that directory (since Go packages are directory-based, not file-based).
5. WHEN `go.mod` is not found in the project root THEN the handler SHALL fall back to treating no imports as project-internal (safe default — no false positives).
6. WHEN an import has an alias (e.g., `import auth "project/internal/auth"`) THEN the handler SHALL extract the path, not the alias.
7. WHEN an import path contains a blank identifier (e.g., `import _ "project/internal/init"`) THEN the handler SHALL still extract the path.
8. WHEN the Go handler is registered THEN it SHALL declare `["go"]` as its supported extensions.
9. WHEN a `.go` file has the `_test.go` suffix THEN the handler SHALL still process it for imports (test files have real dependencies).

### Requirement 2: Go Language Handler — Symbol Extraction (`--symbols`)

**User Story:** As a developer working with Go codebases, I want `src --symbols` to show functions, structs, interfaces, types, constants, and variables declared in Go files, so that I can understand code structure without reading full files.

#### Acceptance Criteria

1. WHEN a `.go` file is processed for `--symbols` THEN the handler SHALL recognize: `func`, `type ... struct`, `type ... interface`, `type` aliases, `const`, `var`, and `func (receiver) method` declarations.
2. WHEN a `func` declaration has a receiver (e.g., `func (s *Server) Start()`) THEN the handler SHALL emit it with kind=method and parent set to the receiver type name (e.g., "Server").
3. WHEN a `func` declaration has no receiver THEN the handler SHALL emit it with kind=fn.
4. WHEN a `type Name struct` is found THEN the handler SHALL emit kind=struct. WHEN `type Name interface` is found THEN kind=interface. WHEN `type Name = OtherType` or `type Name OtherType` THEN kind=type.
5. WHEN a `const` or `var` block is found (e.g., `const ( ... )`) THEN the handler SHALL emit each named constant/variable individually.
6. WHEN a symbol name starts with an uppercase letter THEN the handler SHALL set visibility to "pub" (Go's exported convention). WHEN lowercase, visibility SHALL be None (unexported).
7. WHEN extracting a signature THEN the handler SHALL use the full trimmed declaration line up to the opening `{` or end of line.

### Requirement 3: Python Language Handler — Import Extraction (`--graph`)

**User Story:** As a developer working with Python codebases, I want `src --graph` to show which Python files depend on which other Python files within my project, so that I can understand internal module dependencies.

#### Acceptance Criteria

1. WHEN a `.py` file is processed for `--graph` THEN the handler SHALL recognize: `import module`, `import module.submodule`, `from module import name`, `from module.submodule import name`, and `from . import name` (relative imports).
2. WHEN an import uses relative dot notation (`from . import`, `from .. import`, `from .module import`) THEN the handler SHALL resolve it relative to the importing file's directory.
3. WHEN an import uses an absolute module path THEN the handler SHALL attempt to resolve it to a file within the project by mapping dots to directory separators and checking for both `module.py` and `module/__init__.py`.
4. WHEN an import resolves to a path that does not exist within the project tree THEN the handler SHALL exclude it as an external dependency (e.g., `import os`, `import requests`).
5. WHEN a `from module import name1, name2` statement is found THEN the handler SHALL resolve to the module file, not individual names (since names are symbols within a module, not separate files).
6. WHEN the handler resolves imports THEN it SHALL check for: `module.py`, `module/__init__.py`, and `module/` directory existence within the project file set.
7. WHEN the handler encounters `import module as alias` THEN it SHALL extract the module path, not the alias.
8. WHEN the Python handler is registered THEN it SHALL declare `["py"]` as its supported extension.
9. WHEN a file named `__init__.py` is processed THEN the handler SHALL treat its parent directory as the package and resolve relative imports accordingly.

### Requirement 4: Python Language Handler — Symbol Extraction (`--symbols`)

**User Story:** As a developer working with Python codebases, I want `src --symbols` to show classes, functions, methods, and top-level assignments in Python files, so that I can understand code structure.

#### Acceptance Criteria

1. WHEN a `.py` file is processed for `--symbols` THEN the handler SHALL recognize: `def function_name(`, `class ClassName`, `async def function_name(`, and top-level variable assignments (e.g., `NAME = value`).
2. WHEN a `def` is found with indentation (inside a `class` body) THEN the handler SHALL emit it with kind=method and parent set to the containing class name.
3. WHEN a `def` is found at the top level (no indentation) THEN the handler SHALL emit it with kind=fn.
4. WHEN a `class` declaration is found THEN the handler SHALL emit kind=class.
5. WHEN an `async def` is found THEN the handler SHALL emit the same as `def` (kind=fn or kind=method depending on context).
6. WHEN a function or class name starts with `_` THEN the handler SHALL set visibility to None (convention for private). WHEN a name starts with `__` (dunder) and is a method THEN visibility SHALL be None. All other names SHALL have visibility None (Python has no real access modifiers — we do not fabricate them).
7. WHEN extracting a signature THEN the handler SHALL use the full trimmed declaration line up to and including the `:` at the end.
8. WHEN a decorator line (`@decorator`) precedes a function or class THEN the handler SHALL NOT emit the decorator as a symbol — only the function/class that follows.

### Requirement 5: Integration with Existing `lang/` System

**User Story:** As a maintainer of `src`, I want Go and Python handlers to integrate seamlessly into the existing language system without modifying any files beyond `lang/`.

#### Acceptance Criteria

1. WHEN the Go handler is added THEN the only files modified SHALL be: `src/lang/mod.rs` (add `mod go;` and register in `HANDLERS` / `SYMBOL_HANDLERS`), and the new `src/lang/go.rs`.
2. WHEN the Python handler is added THEN the only files modified SHALL be: `src/lang/mod.rs` (add `mod python;` and register in `HANDLERS` / `SYMBOL_HANDLERS`), and the new `src/lang/python.rs`.
3. WHEN both handlers are registered THEN `--graph` and `--symbols` (if implemented) SHALL automatically pick them up for `.go` and `.py` files with no changes to `graph.rs`, `symbols.rs`, `main.rs`, or `yaml_output.rs`.
4. WHEN a Go project is scanned and `go.mod` needs to be read THEN the handler SHALL read it lazily (only when processing the first `.go` file) and cache the module path for the duration of the scan.
5. WHEN the `scanner.rs` `SOURCE_EXTENSIONS` list is checked THEN `go` and `py` SHALL already be present (they are — both are in the current list).
