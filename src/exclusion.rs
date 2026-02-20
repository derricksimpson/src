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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_exclusions_are_applied() {
        let filter = ExclusionFilter::new(&[], false);
        assert!(filter.is_excluded("node_modules"));
        assert!(filter.is_excluded(".git"));
        assert!(filter.is_excluded("target"));
        assert!(filter.is_excluded("dist"));
        assert!(filter.is_excluded("__pycache__"));
        assert!(filter.is_excluded("build"));
    }

    #[test]
    fn default_exclusions_case_insensitive() {
        let filter = ExclusionFilter::new(&[], false);
        assert!(filter.is_excluded("Node_Modules"));
        assert!(filter.is_excluded("TARGET"));
        assert!(filter.is_excluded(".GIT"));
    }

    #[test]
    fn non_excluded_passes() {
        let filter = ExclusionFilter::new(&[], false);
        assert!(!filter.is_excluded("src"));
        assert!(!filter.is_excluded("lib"));
        assert!(!filter.is_excluded("main.rs"));
    }

    #[test]
    fn additional_exclusions() {
        let filter = ExclusionFilter::new(&["custom_dir".to_owned()], false);
        assert!(filter.is_excluded("custom_dir"));
        assert!(filter.is_excluded("node_modules"));
    }

    #[test]
    fn disable_defaults() {
        let filter = ExclusionFilter::new(&[], true);
        assert!(!filter.is_excluded("node_modules"));
        assert!(!filter.is_excluded(".git"));
        assert!(!filter.is_excluded("target"));
    }

    #[test]
    fn disable_defaults_with_custom() {
        let filter = ExclusionFilter::new(&["only_this".to_owned()], true);
        assert!(!filter.is_excluded("node_modules"));
        assert!(filter.is_excluded("only_this"));
        assert!(filter.is_excluded("ONLY_THIS"));
    }

    #[test]
    fn ascii_lowercase_helper() {
        let mut buf = [0u8; 256];
        assert_eq!(ascii_lowercase("Hello", &mut buf), "hello");
        assert_eq!(ascii_lowercase("UPPER", &mut buf), "upper");
        assert_eq!(ascii_lowercase("lower", &mut buf), "lower");
        assert_eq!(ascii_lowercase("MiXeD", &mut buf), "mixed");
    }
}
