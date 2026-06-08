const MAX_CHARS: usize = 14_000;

const SKIP_NAMES: &[&str] = &[
    "Cargo.lock",
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "poetry.lock",
];

fn should_skip(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    if SKIP_NAMES.contains(&name) {
        return true;
    }
    if path.starts_with("target/") || path.contains("/target/") {
        return true;
    }
    if path.starts_with("dist/") || path.contains("/dist/") {
        return true;
    }
    name.ends_with(".min.js")
        || name.ends_with(".png")
        || name.ends_with(".jpg")
        || name.ends_with(".jpeg")
}

pub fn clean(diff: &str) -> String {
    let mut out = String::new();
    let mut keep = true;
    for line in diff.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            let path = rest
                .split_whitespace()
                .next()
                .and_then(|p| p.strip_prefix("a/"))
                .unwrap_or(rest);
            keep = !should_skip(path);
        }
        if keep {
            out.push_str(line);
            out.push('\n');
        }
    }
    truncate(&out)
}

fn truncate(s: &str) -> String {
    if s.len() <= MAX_CHARS {
        return s.to_string();
    }
    let mut end = MAX_CHARS;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n... [truncated]", &s[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_lockfiles() {
        let diff = "\
diff --git a/Cargo.lock b/Cargo.lock
--- a/Cargo.lock
+++ b/Cargo.lock
+version = 1
diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
+fn main() {}
";
        let cleaned = clean(diff);
        assert!(!cleaned.contains("Cargo.lock"));
        assert!(cleaned.contains("src/main.rs"));
    }

    #[test]
    fn truncates_large_diffs() {
        let body = "x".repeat(MAX_CHARS + 100);
        let diff = format!("diff --git a/foo b/foo\n{body}");
        assert!(clean(&diff).ends_with("... [truncated]"));
    }
}
