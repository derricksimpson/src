use std::collections::HashSet;

const DEFAULT_EXCLUSIONS: &[&str] = &[
    "node_modules", ".git", "bin", "obj", "dist", ".vs",
    "__pycache__", ".idea", ".vscode", ".svn", ".hg",
    "coverage", ".next", ".nuxt", "target", "build",
    "packages", ".cache", ".output", ".parcel-cache",
];

pub struct ExclusionFilter {
    exclusions: HashSet<Box<str>>,
}

impl ExclusionFilter {
    pub fn new(additional: &[String], disable_defaults: bool) -> Self {
        let mut exclusions = HashSet::new();
        if !disable_defaults {
            for &name in DEFAULT_EXCLUSIONS {
                exclusions.insert(name.to_ascii_lowercase().into_boxed_str());
            }
        }
        for name in additional {
            exclusions.insert(name.to_ascii_lowercase().into_boxed_str());
        }
        Self { exclusions }
    }

    pub fn is_excluded(&self, name: &str) -> bool {
        let mut buf = [0u8; 256];
        let lowered = ascii_lowercase(name, &mut buf);
        self.exclusions.contains(lowered)
    }
}

fn ascii_lowercase<'a>(s: &str, buf: &'a mut [u8; 256]) -> &'a str {
    let bytes = s.as_bytes();
    let len = bytes.len().min(256);
    for i in 0..len {
        buf[i] = bytes[i].to_ascii_lowercase();
    }
    unsafe { std::str::from_utf8_unchecked(&buf[..len]) }
}
