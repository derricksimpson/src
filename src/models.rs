use std::fmt;

pub struct MetaInfo {
    pub elapsed_ms: u128,
    pub timeout: bool,
    pub files_scanned: usize,
    pub files_matched: usize,
    pub total_matches: Option<usize>,
}

pub struct FileChunk {
    pub start_line: usize,
    pub end_line: usize,
    pub content: String,
}

pub struct FileEntry {
    pub path: String,
    pub contents: Option<String>,
    pub error: Option<String>,
    pub chunks: Option<Vec<FileChunk>>,
}

pub struct ScanResult {
    pub name: String,
    pub children: Option<Vec<ScanResult>>,
    pub files: Option<Vec<String>>,
}

pub struct GraphEntry {
    pub file: String,
    pub imports: Vec<String>,
}

pub struct SymbolEntry {
    pub kind: String,
    pub name: String,
    pub line: usize,
    pub visibility: Option<String>,
    pub parent: Option<String>,
    pub signature: String,
}

pub struct SymbolFile {
    pub path: String,
    pub symbols: Vec<SymbolEntry>,
    pub error: Option<String>,
}

pub struct CountEntry {
    pub path: String,
    pub count: usize,
}

pub struct LangStats {
    pub extension: String,
    pub files: usize,
    pub lines: usize,
    pub bytes: u64,
}

pub struct LargestFile {
    pub path: String,
    pub lines: usize,
    pub bytes: u64,
}

pub struct StatsTotals {
    pub files: usize,
    pub lines: usize,
    pub bytes: u64,
}

pub struct StatsOutput {
    pub languages: Vec<LangStats>,
    pub totals: StatsTotals,
    pub largest: Vec<LargestFile>,
}

pub struct OutputEnvelope {
    pub meta: Option<MetaInfo>,
    pub files: Option<Vec<FileEntry>>,
    pub tree: Option<ScanResult>,
    pub graph: Option<Vec<GraphEntry>>,
    pub symbols: Option<Vec<SymbolFile>>,
    pub counts: Option<Vec<CountEntry>>,
    pub stats: Option<StatsOutput>,
    pub error: Option<String>,
}

impl fmt::Display for MetaInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "meta:\n")?;
        if self.elapsed_ms != 0 {
            write!(f, "  elapsedMs: {}\n", self.elapsed_ms)?;
        }
        if self.timeout {
            write!(f, "  timeout: true\n")?;
        }
        if self.files_scanned != 0 {
            write!(f, "  filesScanned: {}\n", self.files_scanned)?;
        }
        if self.files_matched != 0 {
            write!(f, "  filesMatched: {}\n", self.files_matched)?;
        }
        if let Some(total) = self.total_matches {
            write!(f, "  totalMatches: {}\n", total)?;
        }
        Ok(())
    }
}
