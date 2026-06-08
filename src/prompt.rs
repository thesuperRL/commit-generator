const DEFAULT_SYSTEM: &str = "You are an expert developer. Review the git diff and write a concise commit message following Conventional Commits (e.g. feat(auth): add JWT validation). Output ONLY the raw commit message — no markdown, no explanation.";

pub fn build_messages(diff: &str, recent: &[String], custom: Option<&str>) -> (String, String) {
    let system = match custom {
        Some(extra) => format!("{DEFAULT_SYSTEM}\n\n{extra}"),
        None => DEFAULT_SYSTEM.to_string(),
    };

    let mut user = String::new();
    if !recent.is_empty() {
        user.push_str("Recent commit subjects in this repo:\n");
        for subject in recent {
            user.push_str("- ");
            user.push_str(subject);
            user.push('\n');
        }
        user.push('\n');
    }
    user.push_str("Staged diff:\n");
    user.push_str(diff);
    (system, user)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn includes_recent_subjects() {
        let (_, user) = build_messages("diff", &["feat: init".into()], None);
        assert!(user.contains("feat: init"));
        assert!(user.contains("Staged diff:"));
    }
}
