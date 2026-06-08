use anyhow::{Context, Result, bail};
use std::path::Path;
use std::process::Command;

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
    let out = Command::new("git")
        .args(["diff", "--staged"])
        .output()
        .context("failed to run git diff --staged")?;
    if !out.status.success() {
        bail!("git diff --staged failed");
    }
    let diff = String::from_utf8_lossy(&out.stdout).into_owned();
    if diff.trim().is_empty() {
        bail!("No staged changes. Run git add first.");
    }
    Ok(diff)
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
