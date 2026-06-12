use anyhow::{Context, Result, bail};
use std::path::Path;
use std::process::Command;

fn git_output(args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .args(args)
        .output()
        .with_context(|| format!("failed to run git {}", args.join(" ")))?;
    if !out.status.success() {
        bail!("git {} failed", args.join(" "));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub fn ensure_repo() -> Result<()> {
    let out = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .context("failed to run git")?;
    if !out.status.success() || String::from_utf8_lossy(&out.stdout).trim() != "true" {
        bail!("not inside a git repository");
    }
    Ok(())
}

pub fn staged_diff() -> Result<String> {
    let diff = git_output(&["diff", "--staged"])?;
    if diff.trim().is_empty() {
        bail!("No staged changes. Run git add first.");
    }
    Ok(diff)
}

pub fn status_short() -> Result<String> {
    git_output(&["status", "-sb"])
}

pub fn staged_name_status() -> Result<String> {
    git_output(&["diff", "--staged", "--name-status"])
}

pub fn staged_paths() -> Result<Vec<String>> {
    Ok(git_output(&["diff", "--staged", "--name-only"])?
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect())
}

const REPO_HINTS: &[&str] = &[
    "README.md",
    "README",
    "Cargo.toml",
    "package.json",
    "pyproject.toml",
    "go.mod",
    "composer.json",
    "Gemfile",
    "Makefile",
];

fn read_git_file(path: &str, staged: bool) -> Option<String> {
    let spec = if staged {
        format!(":{path}")
    } else {
        format!("HEAD:{path}")
    };
    let content = Command::new("git").args(["show", &spec]).output().ok()?;
    if !content.status.success() || content.stdout.contains(&0) {
        return None;
    }
    Some(String::from_utf8_lossy(&content.stdout).into_owned())
}

fn append_file(out: &mut String, path: &str, text: &str, max_chars: usize) -> bool {
    if out.len() >= max_chars {
        return true;
    }
    out.push_str("--- ");
    out.push_str(path);
    out.push_str(" ---\n");
    let remaining = max_chars.saturating_sub(out.len());
    if text.len() > remaining {
        let mut end = remaining;
        while end > 0 && !text.is_char_boundary(end) {
            end -= 1;
        }
        out.push_str(&text[..end]);
        out.push_str("\n... [truncated]\n");
        return true;
    }
    out.push_str(text);
    if !text.ends_with('\n') {
        out.push('\n');
    }
    out.push('\n');
    false
}

pub fn staged_file_contents(max_chars: usize) -> Result<String> {
    let status = staged_name_status()?;
    let mut out = String::new();
    for line in status.lines() {
        if out.len() >= max_chars {
            out.push_str("... [file contents truncated]\n");
            break;
        }
        let Some((tag, path)) = line.split_once('\t') else {
            continue;
        };
        if tag.chars().any(|c| c == 'D') {
            continue;
        }
        let path = path.rsplit(" -> ").next().unwrap_or(path);
        if crate::diff::skip_path(path) {
            continue;
        }
        let Some(text) = read_git_file(path, true) else {
            continue;
        };
        if append_file(&mut out, path, &text, max_chars) {
            break;
        }
    }
    Ok(out)
}

pub fn repo_context_files(staged_paths: &[String], max_chars: usize) -> Result<String> {
    use std::collections::HashSet;

    let mut seen: HashSet<String> = staged_paths.iter().cloned().collect();
    let mut candidates = Vec::new();
    for hint in REPO_HINTS {
        candidates.push(hint.to_string());
    }
    for path in staged_paths {
        let dir = path.rsplit_once('/').map(|(d, _)| d).unwrap_or(".");
        if let Ok(files) = git_output(&["ls-files", dir]) {
            for file in files.lines().take(5) {
                candidates.push(file.to_string());
            }
        }
    }

    let mut out = String::new();
    for path in candidates {
        if !seen.insert(path.clone()) || crate::diff::skip_path(&path) {
            continue;
        }
        let Some(text) = read_git_file(&path, false) else {
            continue;
        };
        if append_file(&mut out, &path, &text, max_chars) {
            out.push_str("... [repo context truncated]\n");
            break;
        }
    }
    Ok(out)
}

pub fn last_commit_context(max_chars: usize) -> Result<String> {
    let head_ok = Command::new("git")
        .args(["rev-parse", "--verify", "HEAD"])
        .output()
        .context("failed to run git rev-parse")?
        .status
        .success();
    if !head_ok {
        return Ok(String::new());
    }

    let mut out = String::new();

    if let Ok(message) = git_output(&["log", "-1", "--format=%B"]) {
        let message = message.trim();
        if !message.is_empty() {
            out.push_str("Previous commit message:\n");
            out.push_str(message);
            out.push('\n');
        }
    }

    let name_status = git_output(&["diff-tree", "--no-commit-id", "--name-status", "-r", "HEAD"])
        .unwrap_or_default();
    if !name_status.trim().is_empty() {
        out.push('\n');
        out.push_str("Previous commit changed files:\n");
        out.push_str(name_status.trim_end());
        out.push('\n');
    }

    let files = last_commit_file_contents(max_chars.saturating_sub(out.len()))?;
    if !files.is_empty() {
        out.push('\n');
        out.push_str("Previous commit file contents:\n");
        out.push_str(&files);
    }

    if let Ok(diff) = git_output(&["show", "HEAD", "--format=", "--patch"]) {
        let diff = crate::diff::clean(&diff);
        if !diff.trim().is_empty() {
            out.push('\n');
            out.push_str("Previous commit diff:\n");
            let remaining = max_chars.saturating_sub(out.len());
            if diff.len() > remaining {
                let mut end = remaining;
                while end > 0 && !diff.is_char_boundary(end) {
                    end -= 1;
                }
                out.push_str(&diff[..end]);
                out.push_str("\n... [previous commit diff truncated]\n");
            } else {
                out.push_str(&diff);
            }
        }
    }

    if out.len() > max_chars {
        let mut end = max_chars;
        while end > 0 && !out.is_char_boundary(end) {
            end -= 1;
        }
        out.truncate(end);
        out.push_str("\n... [previous commit context truncated]\n");
    }

    Ok(out)
}

fn last_commit_file_contents(max_chars: usize) -> Result<String> {
    let status =
        git_output(&["diff-tree", "--no-commit-id", "--name-status", "-r", "HEAD"])?;
    let mut out = String::new();
    for line in status.lines() {
        if out.len() >= max_chars {
            out.push_str("... [previous commit file contents truncated]\n");
            break;
        }
        let Some((tag, path)) = line.split_once('\t') else {
            continue;
        };
        if tag.chars().any(|c| c == 'D') {
            continue;
        }
        let path = path.rsplit(" -> ").next().unwrap_or(path);
        if crate::diff::skip_path(path) {
            continue;
        }
        let Some(text) = read_git_file(path, false) else {
            continue;
        };
        if append_file(&mut out, path, &text, max_chars) {
            break;
        }
    }
    Ok(out)
}

pub fn recent_subjects(n: usize) -> Result<Vec<String>> {
    let out = Command::new("git")
        .args(["log", &format!("-{n}"), "--format=%s"])
        .output()
        .context("failed to run git log")?;
    if !out.status.success() {
        return Ok(vec![]);
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect())
}

pub fn commit(message: &str) -> Result<()> {
    let status = Command::new("git")
        .args(["commit", "-m"])
        .arg(message)
        .status()
        .context("failed to run git commit")?;
    if !status.success() {
        bail!("git commit failed");
    }
    Ok(())
}

pub fn commit_with_editor(message_path: &Path) -> Result<()> {
    let status = Command::new("git")
        .args(["commit", "-e", "-F"])
        .arg(message_path)
        .status()
        .context("failed to run git commit")?;
    if !status.success() {
        bail!("git commit failed or was aborted");
    }
    Ok(())
}
