# Design: `src` CLI Tool

## Architecture Overview

The `src` CLI follows a layered architecture with clear separation of responsibilities.

```
┌─────────────────────────────────────────────────────────┐
│                   Program / Entry                        │
│  (arg parse, subcommand dispatch, exit codes)            │
└─────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────┐
│                      Commands                           │
│  TreeCommand | ListCommand | SearchCommand              │
└─────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────┐
│                      Services                           │
│  FileScanner, ContentSearcher, ExclusionFilter, etc.    │
└─────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────┐
│                       Output                            │
│  YamlOutputWriter, Formatters                           │
└─────────────────────────────────────────────────────────┘
```

---

## Layered Responsibilities

| Layer | Responsibility |
|-------|----------------|
| **Program/Entry** | Parse CLI args, dispatch to subcommands, handle top-level errors, set exit codes |
| **Commands** | Implement command-specific logic, orchestrate services, validate input |
| **Services** | Core business logic: file I/O, pattern matching, exclusion rules |
| **Output** | Serialization, formatting, writing to stdout/stderr |

---

## Component Interfaces

### FileScanner

Scans filesystem and returns file/directory metadata.

```text
interface FileScanner {
  scan(root: string, options: ScanOptions): AsyncIterable<FileEntry>;
}

interface ScanOptions {
  maxDepth?: number;
  includeFiles?: boolean;
  includeDirs?: boolean;
}

interface FileEntry {
  path: string;
  type: 'file' | 'directory';
  size?: number;
  relativePath?: string;
}
```

### ContentSearcher

Searches file contents for patterns.

```text
interface ContentSearcher {
  search(root: string, pattern: string | RegExp, options: SearchOptions): AsyncIterable<SearchMatch>;
}

interface SearchOptions {
  includeGlobs?: string[];
  excludeGlobs?: string[];
  contextBefore?: number;
  contextAfter?: number;
  ignoreCase?: boolean;
}

interface SearchMatch {
  file: string;
  lineNumber: number;
  line: string;
  contextBefore?: string[];
  contextAfter?: string[];
}
```

### ExclusionFilter

Determines which paths to exclude from processing.

```text
interface ExclusionFilter {
  isExcluded(path: string, type: 'file' | 'directory'): boolean;
  addPattern(pattern: string): void;
  loadFromFile(path: string): void;  // e.g., .gitignore
}
```

### GlobMatcher

Matches paths against glob patterns.

```text
interface GlobMatcher {
  match(path: string, pattern: string): boolean;
  matchAny(path: string, patterns: string[]): boolean;
}
```

### YamlOutputWriter

Serializes result structures to YAML.

```text
interface YamlOutputWriter {
  write(tree: TreeResult): void;
  write(files: FileListResult): void;
  write(matches: SearchResult): void;
  flush(): void;
}
```

---

## Data Models

### TreeResult

```yaml
tree:
  root: "."
  entries:
    - name: "src"
      type: "directory"
      children:
        - name: "index.ts"
          type: "file"
        - name: "utils"
          type: "directory"
          children: []
```

### FileListResult

```yaml
files:
  - path: "src/index.ts"
    size: 1024
  - path: "src/utils/helper.ts"
    size: 512
```

### SearchResult

```yaml
matches:
  - file: "src/index.ts"
    line: 5
    content: "export const foo = 1;"
    contextBefore: ["// entry point"]
    contextAfter: ["export const bar = 2;"]
```

---

## Error Handling

| Scenario | Exit Code | Behavior |
|----------|-----------|----------|
| Success | 0 | Normal completion |
| User error (invalid args, file not found) | 1 | Error message to stderr |
| Timeout | 2 | Abort operation, message to stderr |
| SIGINT (Ctrl+C) | 130 | Graceful shutdown |

### Error Handling Strategy

- All user-facing errors emit clear messages to stderr.
- Service-level errors are wrapped with context before reaching the entry point.
- Timeouts are enforced at the orchestration layer (Commands) and propagate exit code 2.

---

## Performance Considerations

1. **Streaming**: File scans and search results stream via `AsyncIterable` to avoid loading entire datasets in memory.

2. **Bounded concurrency**: Use a worker pool (e.g., limited to CPU count) for parallel file reads in content search.

3. **Early exit**: Support `--limit` or similar to stop after N matches when only a subset is needed.

4. **Lazy evaluation**: Build tree structure incrementally; avoid full tree materialization when output is streamed.

5. **Exclusion first**: Apply exclusion rules before expensive I/O (e.g., skip `node_modules` without statting contents).

6. **Index hints** (future): Optional indexing for repeated searches over the same tree.
