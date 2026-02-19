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
