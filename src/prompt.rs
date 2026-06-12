const DEFAULT_SYSTEM: &str = "\
Write a git commit message with a subject line and an optional body. \
Subject: max 30 words, Conventional Commits (e.g. refactor(pkg): rename namespace). \
Body: optional, max 100 words, plain text only. \
Separate subject and body with one blank line. \
No markdown, no bullets, no headings, no code fences. \
Output ONLY the raw commit message.";

pub struct CommitContext<'a> {
    pub status: &'a str,
    pub name_status: &'a str,
    pub repo_files: &'a str,
    pub files: &'a str,
    pub diff: &'a str,
    pub last_commit: &'a str,
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
    if !ctx.last_commit.is_empty() {
        user.push_str("Last commit context:\n");
        user.push_str(ctx.last_commit);
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
    user.push_str("\n\nReply with subject (max 30 words) and optional body (max 100 words).\n");
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
            last_commit: "",
        };
        let (_, user) = build_messages(ctx, &["feat: init".into()], None);
        assert!(user.contains("feat: init"));
        assert!(user.contains("Git status:"));
        assert!(user.contains("Changed files:"));
        assert!(user.contains("Repo context files:"));
        assert!(user.contains("Staged file contents:"));
        assert!(user.contains("Staged diff:"));
    }

    #[test]
    fn includes_last_commit_context() {
        let ctx = CommitContext {
            status: "",
            name_status: "",
            repo_files: "",
            files: "",
            diff: "+change",
            last_commit: "Previous commit message:\nfeat: init\n",
        };
        let (_, user) = build_messages(ctx, &[], None);
        assert!(user.contains("Last commit context:"));
        assert!(user.contains("feat: init"));
    }
}
