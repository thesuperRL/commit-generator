use std::path::PathBuf;

/// Load env from global config, then optional project `.env` in cwd.
pub fn load() {
    if let Ok(home) = std::env::var("HOME") {
        let _ = dotenvy::from_path(PathBuf::from(home).join(".config/git-aicommit/.env"));
    }
    let _ = dotenvy::dotenv();
}
