mod config;
mod diff;
mod env;
mod git;
mod llm;
mod prompt;

use anyhow::Result;
use clap::Parser;
use std::io::Write;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "git-aicommit", about = "AI-generated git commit messages")]
struct Args {
    #[arg(long, env = "AICOMMIT_PROVIDER", default_value = "openrouter")]
    provider: String,
    #[arg(short, long, env = "AICOMMIT_MODEL")]
    model: Option<String>,
    #[arg(long, env = "AICOMMIT_API_KEY")]
    api_key: Option<String>,
    #[arg(long, env = "AICOMMIT_BASE_URL")]
    base_url: Option<String>,
    #[arg(short, long)]
    prompt: Option<PathBuf>,
    #[arg(long)]
    dry_run: bool,
    #[arg(long, short = 'f', conflicts_with = "dry_run")]
    fast: bool,
    #[arg(long, short = 'r', help = "Retry on errors until a valid message is generated")]
    retry_forever: bool,
    #[arg(long, short = 'c', help = "Include the previous commit's contents as context")]
    context: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    env::load();
    let args = Args::parse();
    git::ensure_repo()?;
    let staged_paths = git::staged_paths()?;
    let diff = diff::clean(&git::staged_diff()?);
    let status = git::status_short().unwrap_or_default();
    let name_status = git::staged_name_status().unwrap_or_default();
    let repo_files = git::repo_context_files(&staged_paths, 4_000).unwrap_or_default();
    let files = git::staged_file_contents(6_000).unwrap_or_default();
    let last_commit = if args.context {
        git::last_commit_context(6_000).unwrap_or_default()
    } else {
        String::new()
    };
    let recent = git::recent_subjects(5)?;
    let custom = match args.prompt.as_ref() {
        Some(path) => Some(std::fs::read_to_string(path)?.trim().to_string()),
        None => None,
    };
    let (system, user) = prompt::build_messages(
        prompt::CommitContext {
            status: &status,
            name_status: &name_status,
            repo_files: &repo_files,
            files: &files,
            diff: &diff,
            last_commit: &last_commit,
        },
        &recent,
        custom.as_deref(),
    );
    let cfg = config::Config::from_args(
        &args.provider,
        args.model.as_deref(),
        args.api_key.as_deref(),
        args.base_url.as_deref(),
    )?;
    let message = llm::generate(
        &cfg.base_url,
        &cfg.api_key,
        &cfg.model,
        &system,
        &user,
        args.retry_forever,
    )
    .await?;
    eprintln!("Suggested commit message:\n{message}\n");
    if args.dry_run {
        print!("{message}");
        return Ok(());
    }
    if args.fast {
        git::commit(&message)?;
        return Ok(());
    }
    let mut file = tempfile::NamedTempFile::new()?;
    write!(file, "{message}")?;
    git::commit_with_editor(file.path())?;
    Ok(())
}
