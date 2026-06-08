const DEFAULT_SYSTEM: &str = "You are an expert developer. Review the git status, changed files, file contents, and diff. Write a concise commit message following Conventional Commits (e.g. feat(auth): add JWT validation). Output ONLY the raw commit message — no markdown, no explanation.";

pub struct CommitContext<'a> {
    pub status: &'a str,
    pub name_status: &'a str,
    pub repo_files: &'a str,
    pub files: &'a str,
    pub diff: &'a str,
}

pub fn build_messages(
    ctx: CommitContext<'_>,
    recent: &[String],
    custom: Option<&str>,
) -> (String, String) {
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
    if !ctx.status.is_empty() {
        user.push_str("Git status:\n");
        user.push_str(ctx.status);
        user.push('\n');
    }
    if !ctx.name_status.is_empty() {
        user.push_str("Changed files:\n");
        user.push_str(ctx.name_status);
        user.push('\n');
    }
    if !ctx.repo_files.is_empty() {
        user.push_str("Repo context files:\n");
        user.push_str(ctx.repo_files);
        user.push('\n');
    }
    if !ctx.files.is_empty() {
        user.push_str("Staged file contents:\n");
        user.push_str(ctx.files);
        user.push('\n');
    }
    user.push_str("Staged diff:\n");
    user.push_str(ctx.diff);
    (system, user)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn includes_context_sections() {
        let ctx = CommitContext {
            status: "## main",
            name_status: "M\tsrc/main.rs",
            repo_files: "--- README.md ---\n# App\n",
            files: "--- src/main.rs ---\nfn main() {}\n",
            diff: "+change",
        };
        let (_, user) = build_messages(ctx, &["feat: init".into()], None);
        assert!(user.contains("feat: init"));
        assert!(user.contains("Git status:"));
        assert!(user.contains("Changed files:"));
        assert!(user.contains("Repo context files:"));
        assert!(user.contains("Staged file contents:"));
        assert!(user.contains("Staged diff:"));
    }
}
