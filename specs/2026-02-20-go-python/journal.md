# Journal — Go and Python Language Handlers

## Summary

We added Go and Python to `src`'s language handler system, bringing the supported language count from three to five. Both handlers implement `LangImports` for dependency graph support and `LangSymbols` for symbol extraction. The whole thing touched exactly three files: two new handler files and the `lang/mod.rs` registry — plus one small backward-compatible tweak to `graph.rs` for Go's directory-based imports. Here's how it went.

---

## Phase 1: Go Import Handler (Tasks 1.1–1.5)

Started with the struct and trait boilerplate — `GoImports`, `extensions()` returning `["go"]`, stubbed `extract_imports()`. Straightforward, same shape as every other handler.

The interesting part was `go.mod` discovery. Go's import system is package-path-based — imports like `"github.com/user/project/internal/auth"` only make sense when you know the module root from `go.mod`. We walk up from the file's directory looking for `go.mod`, parse the `module` line, and cache the result in a `OnceLock<Option<String>>`. One filesystem read for the entire scan, regardless of how many `.go` files get processed.

Import parsing uses a simple state machine — `Normal` state looks for `import "path"` (single) or `import (` (start of group block), then `InBlock` state collects paths until the closing `)`. Had to handle aliased imports (`auth "path"`), blank imports (`_ "path"`), and comment lines within the block. Nothing wild, just careful string parsing.

Resolution maps the import path to a project-relative directory: strip the module path prefix from the import, and you've got the internal directory. Returns it with a trailing `/` to signal "this is a directory, not a file" — which brings us to the graph builder change.

Registered in `mod.rs`. Done.

## Phase 2: Graph Builder Directory Support (Task 2.1)

The one change outside `lang/`. Go packages map to directories, not individual files. When `graph.rs` resolves candidates, it normally checks `project_files.contains(&path)` — exact match. For Go, we need prefix matching.

Added a simple check: if the candidate path ends with `/`, iterate over the project file set and collect everything that starts with that prefix. Backward-compatible — no existing handler returns paths ending in `/`, so this codepath is never triggered for Rust, TS, or C#.

Small change, big unlock. Go dependency graphs now show real package-to-file relationships.

## Phase 3: Python Import Handler (Tasks 3.1–3.6)

Python imports are a different beast. Two flavors: absolute (`import myproject.utils.helpers`) and relative (`from ..utils import helper`). Both need different resolution strategies.

Parsing was straightforward — match `import <module>` and `from <module> import <names>` lines, extract the module path, determine if it's relative (starts with `.`), count the dots. Added a basic heuristic to skip lines inside triple-quoted strings so we don't pick up imports from docstrings.

Relative resolution: start from the file's directory, go up `dot_count - 1` levels, append the sub-module path, generate `.py` and `/__init__.py` candidates. The tricky bit was `__init__.py` files — when the importing file itself is `__init__.py`, its "package" is the parent directory, so relative imports resolve from there.

Absolute resolution: replace dots with `/`, generate candidates at each level of the dotted path. `myproject.utils.helpers` becomes candidates for `myproject/utils/helpers.py`, `myproject/utils/helpers/__init__.py`, `myproject/utils.py`, etc. External packages like `os` or `requests` naturally don't match any project file and get filtered by the graph builder's `project_files` check. No need for an external package list.

Registered in `mod.rs`. The graph now works for Python projects.

## Phase 4: Go Symbol Extraction (Tasks 4.1–4.2)

Implemented `LangSymbols` for `GoImports`. Go's declaration syntax is clean and consistent, which made this pleasant:

- `func Name(` → kind=fn
- `func (r *Type) Name(` → kind=method, parent=Type. Extracting the receiver type means parsing the `(r *Type)` block — strip the variable name and pointer star, grab the type name.
- `type Name struct {` → kind=struct
- `type Name interface {` → kind=interface
- `type Name = Other` → kind=type
- `const` and `var` blocks: track parenthesized blocks, extract each named entry

Go's visibility is beautifully simple — uppercase first letter means exported. We map that to `visibility: "pub"`. Lowercase gets None.

Registered in `SYMBOL_HANDLERS`.

## Phase 5: Python Symbol Extraction (Tasks 5.1–5.2)

The class-method tracking was the main complexity here. Python uses indentation to define scope, so we track the current class context and its indentation level. When we see `class ClassName:` at indent N, every subsequent `def` at indent > N is a method of that class. When indentation drops back to N or below, we leave the class context.

- `def name(` at indent 0 → kind=fn
- `def name(` inside a class → kind=method, parent=class
- `async def name(` → same treatment
- `class ClassName:` → kind=class
- `UPPER_SNAKE = value` at indent 0 → kind=const

Decorators (`@something`) are deliberately skipped — they annotate the next declaration, they aren't declarations themselves.

Python has no real visibility system, so every symbol gets `visibility: None`. We considered mapping `_` prefix to "private" but decided against it — it's a convention, not an access modifier, and reporting it as a visibility level would be misleading.

Registered in `SYMBOL_HANDLERS`.

---

## What We Shipped

Two new language handlers that slot cleanly into the existing `lang/` trait system:

- `src --graph --r *.go` — Go dependency graph showing package-to-file relationships, powered by `go.mod` module resolution
- `src --graph --r *.py` — Python dependency graph with both relative and absolute import resolution
- `src --symbols --r *.go` — Go symbol extraction: funcs, methods, structs, interfaces, types, consts
- `src --symbols --r *.py` — Python symbol extraction: functions, methods, classes, constants

Files changed: `lang/mod.rs` (registration), `lang/go.rs` (new), `lang/python.rs` (new), `graph.rs` (one prefix-match addition). Everything else — scanner, YAML output, CLI, main dispatch — untouched. That's the beauty of the pluggable trait system.
