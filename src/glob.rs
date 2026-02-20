/// Simple glob matching (case-insensitive) supporting `*` and `?`.
/// This matches the behavior of .NET's `FileSystemName.MatchesSimpleExpression`.
pub fn matches(name: &str, pattern: &str) -> bool {
    glob_match(
        name.as_bytes(),
        pattern.as_bytes(),
        0,
        0,
    )
}

pub fn matches_any(name: &str, patterns: &[String]) -> bool {
    for pattern in patterns {
        if matches(name, pattern) {
            return true;
        }
    }
    false
}

fn glob_match(name: &[u8], pattern: &[u8], mut ni: usize, mut pi: usize) -> bool {
    let mut star_pi = usize::MAX;
    let mut star_ni = 0;

    while ni < name.len() {
        if pi < pattern.len() {
            let p = pattern[pi];
            let n = name[ni];
            if p == b'?' || eq_ci(p, n) {
                pi += 1;
                ni += 1;
                continue;
            }
            if p == b'*' {
                star_pi = pi;
                star_ni = ni;
                pi += 1;
                continue;
            }
        }
        if star_pi != usize::MAX {
            pi = star_pi + 1;
            star_ni += 1;
            ni = star_ni;
            continue;
        }
        return false;
    }

    while pi < pattern.len() && pattern[pi] == b'*' {
        pi += 1;
    }
    pi == pattern.len()
}

#[inline(always)]
fn eq_ci(a: u8, b: u8) -> bool {
    a.to_ascii_lowercase() == b.to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_match() {
        assert!(matches("hello.rs", "hello.rs"));
    }

    #[test]
    fn star_matches_extension() {
        assert!(matches("main.rs", "*.rs"));
    }

    #[test]
    fn star_matches_prefix() {
        assert!(matches("main.rs", "main.*"));
    }

    #[test]
    fn star_matches_all() {
        assert!(matches("anything.txt", "*.*"));
    }

    #[test]
    fn star_matches_everything() {
        assert!(matches("anything", "*"));
    }

    #[test]
    fn question_mark_single_char() {
        assert!(matches("abc", "a?c"));
        assert!(!matches("abbc", "a?c"));
    }

    #[test]
    fn case_insensitive() {
        assert!(matches("Main.RS", "*.rs"));
        assert!(matches("HELLO.rs", "hello.rs"));
    }

    #[test]
    fn no_match() {
        assert!(!matches("hello.rs", "*.ts"));
        assert!(!matches("a", "ab"));
    }

    #[test]
    fn empty_pattern_only_matches_empty() {
        assert!(matches("", ""));
        assert!(!matches("a", ""));
    }

    #[test]
    fn star_matches_empty() {
        assert!(matches("", "*"));
    }

    #[test]
    fn multiple_stars() {
        assert!(matches("abcdef", "*cd*"));
        assert!(matches("abcdef", "a*f"));
        assert!(matches("abcdef", "a*c*f"));
    }

    #[test]
    fn matches_any_works() {
        let patterns: Vec<String> = vec!["*.rs".into(), "*.ts".into()];
        assert!(matches_any("file.rs", &patterns));
        assert!(matches_any("file.ts", &patterns));
        assert!(!matches_any("file.py", &patterns));
    }

    #[test]
    fn matches_any_empty_patterns() {
        let patterns: Vec<String> = vec![];
        assert!(!matches_any("file.rs", &patterns));
    }

    #[test]
    fn complex_glob_patterns() {
        assert!(matches("test_file.spec.ts", "*.ts"));
        assert!(matches("test_file.spec.ts", "*.spec.ts"));
        assert!(matches("a.b.c.d", "a.*.d"));
    }
}
