# Design Document — Go and Python Language Handlers

## Overview

This design adds two new language handlers to `src`'s `lang/` module: Go and Python. Each handler implements `LangImports` (for `--graph`) and optionally `LangSymbols` (for `--symbols`, if that feature has been built — see `specs/2026-02-20-symbols`). The handlers follow the exact same pattern as the existing Rust, TypeScript, and C# handlers.

The key design challenge for each language is import resolution — mapping language-specific import syntax to actual file paths within the project:

- **Go** is directory-based: imports reference package paths, not files. Requires reading `go.mod` to determine the module root path.
- **Python** has both absolute and relative imports, with module resolution mapping dots to directories and checking for both `module.py` and `module/__init__.py`.

## Architecture

No new modules or architectural changes. Everything slots into the existing `lang/` trait system:

```
src/lang/
  mod.rs         — Updated: add `mod go;`, `mod python;`, register in HANDLERS
  rust.rs        — Unchanged
  typescript.rs  — Unchanged
  csharp.rs      — Unchanged
  go.rs          — NEW: GoImports implementing LangImports (+ LangSymbols)
  python.rs      — NEW: PythonImports implementing LangImports (+ LangSymbols)
```

The graph builder (`graph.rs`), scanner, YAML output, and main dispatch are completely untouched. They work with `&dyn LangImports` — they don't know or care which languages exist.

## Components and Interfaces

### 1. Go Handler (`lang/go.rs`)

#### Struct

```rust
pub struct GoImports;
```

#### `LangImports` Implementation

```rust
impl LangImports for GoImports {
    fn extensions(&self) -> &[&str] {
        &["go"]
    }

    fn extract_imports(&self, content: &str, file_path: &Path) -> Vec<String> {
        // 1. Find go.mod relative to file_path (walk up directories)
        // 2. Extract module path from go.mod (e.g., "github.com/user/project")
        // 3. Parse import statements from content
        // 4. Filter to imports whose path starts with the module prefix
        // 5. Map to relative directory paths within the project
        // 6. Expand directory to all .go files in that directory
    }
}
```

#### Go Import Syntax

Single import:
```go
import "github.com/user/project/internal/auth"
```

Grouped import:
```go
import (
    "fmt"
    "net/http"

    auth "github.com/user/project/internal/auth"
    _ "github.com/user/project/internal/init"
    "github.com/user/project/pkg/utils"
)
```

#### Parsing Strategy

Line-by-line scan with a state machine:
- State `Normal`: look for `import "path"` (single) or `import (` (start of group)
- State `InImportBlock`: collect paths until `)`, handling aliases, blank identifiers, and comments

For each import line in a group:
1. Strip leading whitespace
2. Skip empty lines and comment lines (`//`)
3. If line starts with `_` or a word followed by a space and `"`, extract the quoted path
4. Otherwise extract the quoted path directly

#### Resolution Strategy

```
Import path: "github.com/user/project/internal/auth"
Module path: "github.com/user/project"
               ↓ strip module prefix
Relative dir:  "internal/auth"
               ↓ find all .go files in that dir
Resolved:      ["internal/auth/handler.go", "internal/auth/middleware.go", ...]
```

The handler needs access to the project file set to enumerate files in the target directory. Since `extract_imports` receives `file_path` but not the file set, the resolution to specific `.go` files happens in `graph.rs` during the resolution phase — the handler returns directory paths (e.g., `internal/auth/`) and `graph.rs` matches them against the project file set.

Alternatively, the handler returns all candidate paths (the directory path with a `*.go` glob-like expansion). Since the graph builder already checks resolved paths against `project_files`, returning candidates like `internal/auth/*.go` won't work directly. Instead, the handler should return the directory path and we check if any project file starts with that prefix.

**Chosen approach**: The handler returns paths in the form `internal/auth/` (trailing slash = directory). The graph builder in `graph.rs::process_file` already does resolution via `project_files.contains(&normalized)`. We extend this check: for candidates ending in `/`, match any project file that starts with that prefix. This is a small, targeted change to `graph.rs`.

#### `go.mod` Reading

The handler needs the module path from `go.mod`. Since `extract_imports` is called per-file and `go.mod` doesn't change between files, we use a lazy `OnceLock`:

```rust
use std::sync::OnceLock;

static GO_MODULE_PATH: OnceLock<Option<String>> = OnceLock::new();

fn get_module_path(file_path: &Path) -> Option<&str> {
    GO_MODULE_PATH.get_or_init(|| {
        find_and_parse_go_mod(file_path)
    }).as_deref()
}
```

Walk up from `file_path` looking for `go.mod`. Parse the `module` line:
```
module github.com/user/project
```
Extract everything after `module `.

#### `LangSymbols` Implementation (if symbols spec is completed)

Detects:
- `func Name(` → kind=fn, visibility based on first letter capitalization
- `func (r *Type) Name(` → kind=method, parent=Type
- `type Name struct {` → kind=struct
- `type Name interface {` → kind=interface
- `type Name = Other` / `type Name Other` → kind=type
- `const Name =` / `const ( Name = ... )` → kind=const
- `var Name =` / `var ( Name = ... )` → kind=var

Visibility: uppercase first letter = "pub", lowercase = None.

### 2. Python Handler (`lang/python.rs`)

#### Struct

```rust
pub struct PythonImports;
```

#### `LangImports` Implementation

```rust
impl LangImports for PythonImports {
    fn extensions(&self) -> &[&str] {
        &["py"]
    }

    fn extract_imports(&self, content: &str, file_path: &Path) -> Vec<String> {
        // 1. Parse import statements
        // 2. Resolve relative imports based on file_path
        // 3. Resolve absolute imports by mapping dots to directories
        // 4. Return candidate file paths
    }
}
```

#### Python Import Syntax

```python
import os                          # external — skip
import myproject.utils.helpers     # absolute internal — resolve
from myproject.utils import helpers  # absolute internal — resolve
from . import sibling              # relative — resolve from current package
from .. import parent_module       # relative — resolve from parent package
from .submodule import func        # relative — resolve from current package subdir
import myproject.utils as utils    # aliased — extract module path, not alias
from typing import List            # external — skip
```

#### Parsing Strategy

Line-by-line scan:
1. Match `import <module>` — extract module path (before `as` if aliased)
2. Match `from <module> import <names>` — extract module path only (names are symbols, not files)
3. For `from . import ...` / `from .. import ...` / `from .sub import ...` — count leading dots for relative depth, extract the rest as the sub-module path

#### Resolution Strategy

**Relative imports** (`from .` / `from ..`):
```
File:          src/pkg/handlers/api.py
Import:        from ..utils import helper
Dot count:     2 (go up 2 levels from file's package)
Base dir:      src/pkg/  (2 levels up from src/pkg/handlers/)
Sub-module:    utils
Candidates:    ["src/pkg/utils.py", "src/pkg/utils/__init__.py"]
```

Resolution of dot-relative imports:
1. Start from the importing file's directory
2. Go up `dot_count - 1` directories (one dot = current package, two dots = parent, etc.)
3. Append the sub-module path, mapping dots to `/`
4. Check for `path.py` and `path/__init__.py`

**Absolute imports** (`import foo.bar`):
```
Import:        myproject.utils.helpers
Candidates:    ["myproject/utils/helpers.py", "myproject/utils/helpers/__init__.py",
                "myproject/utils.py", "myproject.py"]
```

We generate candidates by progressively mapping dot-separated segments to directory paths and trying both `.py` and `/__init__.py`. The graph builder's `project_files.contains()` check filters to only those that actually exist.

Since we don't know the project's package name, we try the full dotted path as-is. If the project root contains a directory matching the first segment, it'll resolve. External packages (like `os`, `requests`) naturally won't match any project file and get filtered out.

#### `LangSymbols` Implementation (if symbols spec is completed)

Detects:
- `def name(` at indent 0 → kind=fn
- `def name(` at indent > 0 (inside class) → kind=method, parent=class name
- `async def name(` → same as `def`
- `class ClassName` → kind=class
- `NAME = value` at indent 0, where NAME is UPPER_SNAKE → kind=const

Class tracking: when we see `class ClassName`, record it and track indentation. Subsequent `def` lines with greater indentation are methods of that class. When indentation returns to class level or lower, reset.

Visibility: Python has no enforced visibility. We set visibility to None for everything. The `_` prefix convention is a hint, not an access modifier.

### 3. Registration (`lang/mod.rs`)

Add to the top of the file:
```rust
mod go;
mod python;
```

Add to `HANDLERS`:
```rust
static HANDLERS: &[&dyn LangImports] = &[
    &rust::RustImports,
    &typescript::TypeScriptImports,
    &csharp::CSharpImports,
    &go::GoImports,
    &python::PythonImports,
];
```

And if `SYMBOL_HANDLERS` exists:
```rust
static SYMBOL_HANDLERS: &[&dyn LangSymbols] = &[
    &rust::RustImports,
    &typescript::TypeScriptImports,
    &csharp::CSharpImports,
    &go::GoImports,
    &python::PythonImports,
];
```

### 4. Graph Builder Change (`graph.rs`)

One small change to support Go's directory-based imports. In `process_file`, when checking candidates against `project_files`, add a prefix-match fallback for candidates ending in `/`:

```rust
// In process_file, where we check project_files.contains(&normalized):
if normalized.ends_with('/') {
    // Directory import (Go packages) — match any file under this prefix
    for pf in project_files.iter() {
        if pf.starts_with(&normalized) && seen.insert(pf.clone()) {
            resolved.push(pf.clone());
        }
    }
} else {
    // Exact file match (existing behavior)
    if project_files.contains(&normalized) && seen.insert(normalized.clone()) {
        resolved.push(normalized);
    }
}
```

This is the only change outside `lang/` — and it's backward-compatible since no existing handler returns paths ending in `/`.

## Data Models

No new data models. The handlers produce `Vec<String>` (import paths) via `LangImports` and `Vec<SymbolInfo>` via `LangSymbols` — both already defined.

## Error Handling

| Scenario | Behavior |
|----------|----------|
| `go.mod` not found | No Go imports treated as internal (empty imports list for all Go files) |
| `go.mod` malformed (no `module` line) | Same as not found |
| Python relative import with too many dots (exceeds project root) | Candidate resolves to path outside project → filtered out by `project_files` check |
| Python `__init__.py` in a deeply nested package | Relative imports resolve from `__init__.py`'s parent directory |
| `.go` / `.py` file is binary | Already handled by graph builder (read error → empty imports) |
| `.go` / `.py` file has permission error | Already handled by graph builder (read error → `GraphEntry` with empty imports) |
