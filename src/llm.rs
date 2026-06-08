use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: Option<String>,
}

pub async fn complete(
    base_url: &str,
    api_key: &str,
    model: &str,
    system: &str,
    user: &str,
) -> Result<String> {
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let resp = reqwest::Client::new()
        .post(&url)
        .bearer_auth(api_key)
        .json(&ChatRequest {
            model: model.to_string(),
            messages: vec![
                Message {
                    role: "system".into(),
                    content: system.to_string(),
                },
                Message {
                    role: "user".into(),
                    content: user.to_string(),
                },
            ],
            temperature: 0.2,
            max_tokens: 150,
        })
        .send()
        .await
        .context("LLM request failed")?;

    if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        bail!("rate limited (429). Retry later or try --provider openrouter");
    }
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        bail!("LLM error {status}: {body}");
    }

    let data: ChatResponse = resp.json().await.context("invalid LLM response")?;
    let raw = data
        .choices
        .first()
        .and_then(|c| c.message.content.as_deref())
        .context("empty LLM response")?;
    Ok(clean_message(raw))
}

fn clean_message(s: &str) -> String {
    let s = s.trim();
    if s.starts_with("```") {
        let inner = s
            .trim_start_matches("```")
            .trim_start_matches("markdown")
            .trim();
        if let Some(end) = inner.rfind("```") {
            return inner[..end].trim().to_string();
        }
    }
    s.to_string()
}
