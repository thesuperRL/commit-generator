use anyhow::{Context, Result};

pub struct Config {
    pub model: String,
    pub api_key: String,
    pub base_url: String,
}

impl Config {
    pub fn from_args(
        provider: &str,
        model: Option<&str>,
        api_key: Option<&str>,
        base_url: Option<&str>,
    ) -> Result<Self> {
        let (default_model, key_env, default_base) = match provider {
            "openrouter" => (
                "google/gemma-4-31b-it:free",
                "OPENROUTER_API_KEY",
                "https://openrouter.ai/api/v1",
            ),
            _ => (
                "gemini-2.5-flash",
                "GEMINI_API_KEY",
                "https://generativelanguage.googleapis.com/v1beta/openai/",
            ),
        };

        Ok(Self {
            model: model
                .map(str::to_owned)
                .or_else(|| std::env::var("AICOMMIT_MODEL").ok())
                .unwrap_or_else(|| default_model.to_string()),
            base_url: base_url
                .map(str::to_owned)
                .or_else(|| std::env::var("AICOMMIT_BASE_URL").ok())
                .unwrap_or_else(|| default_base.to_string()),
            api_key: api_key
                .map(str::to_owned)
                .or_else(|| std::env::var("AICOMMIT_API_KEY").ok())
                .or_else(|| std::env::var(key_env).ok())
                .with_context(|| format!("missing API key: set {key_env} or use --api-key"))?,
        })
    }
}
