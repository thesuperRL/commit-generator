use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

const MAX_SUBJECT_WORDS: usize = 30;
const MAX_BODY_WORDS: usize = 100;
const MAX_ATTEMPTS: usize = 3;

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

pub async fn generate(
    base_url: &str,
    api_key: &str,
    model: &str,
    system: &str,
    user: &str,
    retry_forever: bool,
) -> Result<String> {
    let mut user_prompt = user.to_string();
    let mut last_issue = String::new();
    let mut attempt = 0u64;

    loop {
        attempt += 1;
        if !retry_forever && attempt > MAX_ATTEMPTS as u64 {
            bail!("failed after {MAX_ATTEMPTS} attempts: {last_issue}");
        }

        let raw = match complete_once(base_url, api_key, model, system, &user_prompt).await {
            Ok(raw) => raw,
            Err(e) => {
                last_issue = error_summary(&e);
                if retry_forever {
                    eprintln!("retry {attempt}: {last_issue}");
                    continue;
                }
                return Err(e);
            }
        };

        match validate(&raw) {
            Ok(message) => return Ok(message),
            Err(issue) => {
                last_issue = issue.clone();
                if retry_forever {
                    eprintln!("retry {attempt}: validation: {issue}");
                }
                user_prompt.push_str(&format!(
                    "\n\nPrevious attempt rejected ({issue}). \
                     Reply again: subject max {MAX_SUBJECT_WORDS} words, optional body max {MAX_BODY_WORDS} words.\n"
                ));
            }
        }
    }
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
}
