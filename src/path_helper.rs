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
