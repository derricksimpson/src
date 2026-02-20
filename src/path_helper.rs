use std::path::Path;

pub fn normalized_relative(root: &Path, full: &Path) -> String {
    match full.strip_prefix(root) {
        Ok(rel) => {
            let s = rel.to_string_lossy();
            if cfg!(windows) {
                s.replace('\\', "/")
            } else {
                s.into_owned()
            }
        }
        Err(_) => full.to_string_lossy().into_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_relative_path() {
        let root = Path::new("/project");
        let full = Path::new("/project/src/main.rs");
        let result = normalized_relative(root, full);
        assert_eq!(result, "src/main.rs");
    }

    #[test]
    fn path_equals_root() {
        let root = Path::new("/project");
        let full = Path::new("/project");
        let result = normalized_relative(root, full);
        assert_eq!(result, "");
    }

    #[test]
    fn path_outside_root_returns_full() {
        let root = Path::new("/project");
        let full = Path::new("/other/file.rs");
        let result = normalized_relative(root, full);
        assert!(result.contains("other"));
        assert!(result.contains("file.rs"));
    }

    #[test]
    fn nested_deep_path() {
        let root = Path::new("/project");
        let full = Path::new("/project/src/lang/rust.rs");
        let result = normalized_relative(root, full);
        assert_eq!(result, "src/lang/rust.rs");
    }
}
