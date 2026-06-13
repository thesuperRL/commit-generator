use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;

const MAX_SUBJECT_WORDS: usize = 30;
const MAX_BODY_WORDS: usize = 100;
const MAX_ATTEMPTS: usize = 3;
const FALLBACK_TRIES_PER_MODEL: u64 = 3;

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

#[derive(Deserialize)]
struct ModelsResponse {
    data: Vec<ModelInfo>,
}

#[derive(Deserialize)]
struct ModelInfo {
    id: String,
    pricing: ModelPricing,
}

#[derive(Deserialize)]
struct ModelPricing {
    prompt: String,
    completion: String,
}

pub async fn generate(
    base_url: &str,
    api_key: &str,
    model: &str,
    system: &str,
    user: &str,
    retry_forever: bool,
    openrouter_fallbacks: bool,
) -> Result<String> {
    let mut user_prompt = user.to_string();
    let mut last_issue = String::new();
    let mut attempt = 0u64;
    let mut fallback_models: Option<Vec<String>> = None;
    let mut fallback_fetch: Option<JoinHandle<Result<Vec<String>>>> = None;
    let mut fallback_index = 0usize;
    let mut tries_on_fallback = 0u64;

    loop {
        if let Some(handle) = fallback_fetch.as_ref() {
            if handle.is_finished() {
                let handle = fallback_fetch.take().unwrap();
                if let Ok(Ok(models)) = handle.await {
                    if !models.is_empty() {
                        eprintln!(
                            "loaded {} free fallback models (rotate every {FALLBACK_TRIES_PER_MODEL} tries)",
                            models.len()
                        );
                        fallback_models = Some(models);
                    }
                }
            }
        }

        attempt += 1;
        if !retry_forever && attempt > MAX_ATTEMPTS as u64 {
            bail!("failed after {MAX_ATTEMPTS} attempts: {last_issue}");
        }

        let current_model = fallback_models
            .as_ref()
            .filter(|m| !m.is_empty())
            .map(|m| m[fallback_index % m.len()].clone())
            .unwrap_or_else(|| model.to_string());
        let on_primary = fallback_models.is_none();

        let raw = match complete_once(base_url, api_key, &current_model, system, &user_prompt).await
        {
            Ok(raw) => raw,
            Err(e) => {
                last_issue = error_summary(&e);
                if retry_forever {
                    if openrouter_fallbacks
                        && is_rate_limited(&e)
                        && on_primary
                        && fallback_fetch.is_none()
                    {
                        eprintln!("rate limited; fetching free fallback models...");
                        let base_url = base_url.to_string();
                        let api_key = api_key.to_string();
                        fallback_fetch = Some(tokio::spawn(async move {
                            fetch_free_fallback_models(&base_url, &api_key).await
                        }));
                    }
                    eprintln!("retry {attempt} ({current_model}): {last_issue}");
                    rotate_fallback(&fallback_models, &mut fallback_index, &mut tries_on_fallback);
                    continue;
                }
                return Err(e);
            }
        };

        match validate(&raw) {
            Ok(message) => {
                eprintln!("succeeded with model: {current_model}");
                return Ok(message);
            }
            Err(issue) => {
                last_issue = issue.clone();
                if retry_forever {
                    eprintln!("retry {attempt} ({current_model}): validation: {issue}");
                    rotate_fallback(&fallback_models, &mut fallback_index, &mut tries_on_fallback);
                }
                user_prompt.push_str(&format!(
                    "\n\nPrevious attempt rejected ({issue}). \
                     Reply again: subject max {MAX_SUBJECT_WORDS} words, optional body max {MAX_BODY_WORDS} words.\n"
                ));
            }
        }
    }
}

fn rotate_fallback(
    models: &Option<Vec<String>>,
    index: &mut usize,
    tries: &mut u64,
) {
    let Some(list) = models.as_ref().filter(|m| !m.is_empty()) else {
        return;
    };
    *tries += 1;
    if *tries >= FALLBACK_TRIES_PER_MODEL {
        *tries = 0;
        *index = (*index + 1) % list.len();
        eprintln!("rotating to fallback model: {}", list[*index]);
    }
}

async fn fetch_free_fallback_models(base_url: &str, api_key: &str) -> Result<Vec<String>> {
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let resp = reqwest::Client::new()
        .get(&url)
        .bearer_auth(api_key)
        .send()
        .await
        .context("failed to fetch OpenRouter models")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        bail!("models API error {status}: {body}");
    }

    let data: ModelsResponse = resp.json().await.context("invalid models response")?;
    let mut models: Vec<String> = data
        .data
        .into_iter()
        .filter(|m| is_free_fallback(&m.id, &m.pricing))
        .map(|m| m.id)
        .collect();
    models.sort_by_cached_key(|id| fallback_speed_rank(id));
    models.dedup();
    Ok(models)
}

fn fallback_speed_rank(id: &str) -> u64 {
    let id = id.to_lowercase();
    if id.contains("haiku")
        || id.contains("flash")
        || id.contains("lite")
        || id.contains("mini")
        || id.contains("nano")
    {
        return 0;
    }
    extract_param_b(&id).unwrap_or_else(|| {
        if id.contains("opus") || id.contains("pro") {
            10_000
        } else {
            5_000
        }
    })
}

fn extract_param_b(id: &str) -> Option<u64> {
    id.split(|c: char| !c.is_ascii_alphanumeric() && c != '.')
        .filter_map(|part| {
            let n: f64 = part.strip_suffix('b')?.parse().ok()?;
            Some(n.ceil() as u64)
        })
        .min()
}

fn is_free_fallback(id: &str, pricing: &ModelPricing) -> bool {
    if pricing.prompt != "0" || pricing.completion != "0" {
        return false;
    }
    let id = id.to_lowercase();
    id.contains("gemini")
        || id.contains("claude")
        || id.starts_with("openai/")
        || id.starts_with("meta-llama/")
        || id.starts_with("meta/")
}

fn is_rate_limited(err: &anyhow::Error) -> bool {
    err.to_string().contains("rate limited (429)")
}

fn error_summary(err: &anyhow::Error) -> String {
    one_line(&err.to_string())
}

fn one_line(s: &str) -> String {
    let line = s.lines().next().unwrap_or(s).trim();
    if line.len() > 120 {
        format!("{}...", &line[..117])
    } else {
        line.to_string()
    }
}

async fn complete_once(
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
            max_tokens: 200,
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
    data.choices
        .first()
        .and_then(|c| c.message.content.clone())
        .context("empty LLM response")
}

fn word_count(s: &str) -> usize {
    s.split_whitespace().count()
}

fn validate(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("empty message".into());
    }
    if trimmed.contains("```") {
        return Err("contains markdown code fences".into());
    }

    let lines: Vec<&str> = trimmed.lines().collect();
    let Some(subject) = lines
        .first()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
    else {
        return Err("empty subject".into());
    };

    if subject.starts_with('#') || subject.starts_with("* ") || subject.starts_with("- ") {
        return Err("subject contains markdown headings or bullets".into());
    }

    let subject_words = word_count(subject);
    if subject_words > MAX_SUBJECT_WORDS {
        return Err(format!(
            "subject exceeded {MAX_SUBJECT_WORDS} words (got {subject_words})"
        ));
    }

    let mut body_start = 1;
    while body_start < lines.len() && lines[body_start].trim().is_empty() {
        body_start += 1;
    }

    if body_start >= lines.len() {
        return Ok(subject.to_string());
    }

    if body_start == 1 {
        return Err("body must be separated from subject by a blank line".into());
    }

    let body = lines[body_start..].join("\n").trim().to_string();
    let body_words = word_count(&body);
    if body_words > MAX_BODY_WORDS {
        return Err(format!(
            "body exceeded {MAX_BODY_WORDS} words (got {body_words})"
        ));
    }

    Ok(format!("{subject}\n\n{body}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_subject_only() {
        assert_eq!(
            validate("refactor: rename package").unwrap(),
            "refactor: rename package"
        );
    }

    #[test]
    fn accepts_subject_and_body() {
        let msg = "refactor: rename package\n\nUpdates imports and Gradle namespace.";
        assert_eq!(validate(msg).unwrap(), msg);
    }

    #[test]
    fn rejects_long_subject() {
        let subject = (0..31).map(|i| format!("w{i}")).collect::<Vec<_>>().join(" ");
        assert!(validate(&subject).unwrap_err().contains("subject exceeded"));
    }

    #[test]
    fn rejects_long_body() {
        let body = (0..101).map(|i| format!("w{i}")).collect::<Vec<_>>().join(" ");
        let msg = format!("refactor: rename package\n\n{body}");
        assert!(validate(&msg).unwrap_err().contains("body exceeded"));
    }

    #[test]
    fn rejects_body_without_blank_line() {
        assert!(validate("subject line\nbody line")
            .unwrap_err()
            .contains("blank line"));
    }

    #[test]
    fn rejects_markdown_subject() {
        assert!(validate("### Summary").unwrap_err().contains("markdown"));
    }

    #[test]
    fn ranks_smaller_models_first() {
        assert!(fallback_speed_rank("meta-llama/llama-3.2-3b-instruct:free")
            < fallback_speed_rank("openai/gpt-oss-20b:free"));
        assert!(fallback_speed_rank("openai/gpt-oss-20b:free")
            < fallback_speed_rank("meta-llama/llama-3.3-70b-instruct:free"));
    }
}
