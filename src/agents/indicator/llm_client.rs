use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use thiserror::Error;

const DEFAULT_TEMPERATURE: f64 = 0.3;
const DEFAULT_MAX_TOKENS: u32 = 512;
const DEFAULT_TIMEOUT_SECS: u64 = 30;
const DEFAULT_MAX_RETRIES: u32 = 2;
const RATE_LIMIT_INTERVAL: Duration = Duration::from_millis(200);

/// Configuration for the LLM endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub base_url: String,
    pub model: String,
    #[serde(default = "default_temperature")]
    pub temperature: f64,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    /// Optional path to an external system prompt file.
    /// If omitted or the file cannot be read, the embedded prompt is used.
    pub prompt_path: Option<String>,
}

fn default_temperature() -> f64 {
    DEFAULT_TEMPERATURE
}
fn default_max_tokens() -> u32 {
    DEFAULT_MAX_TOKENS
}
fn default_timeout_secs() -> u64 {
    DEFAULT_TIMEOUT_SECS
}
fn default_max_retries() -> u32 {
    DEFAULT_MAX_RETRIES
}

#[derive(Debug, Error)]
pub enum LlmError {
    #[error("LLM request timed out after {0}s")]
    Timeout(u64),
    #[error("LLM returned empty response")]
    EmptyResponse,
    #[error("LLM API error: HTTP {status}: {body}")]
    Api { status: u16, body: String },
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
}

// ── OpenAI-compatible request/response types ────────────────────

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f64,
    max_tokens: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

/// HTTP client for OpenAI-compatible chat completions (Ollama/SGLang).
pub struct LlmClient {
    http: reqwest::Client,
    config: LlmConfig,
    last_request: Option<Instant>,
}

impl LlmClient {
    pub fn new(config: LlmConfig) -> Result<Self, LlmError> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .map_err(LlmError::Http)?;

        Ok(Self {
            http,
            config,
            last_request: None,
        })
    }

    /// For testing: construct with `.no_proxy()` to bypass HTTP proxy on cluster login nodes.
    #[cfg(test)]
    pub fn new_test(config: LlmConfig) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .no_proxy()
            .build()
            .unwrap();

        Self {
            http,
            config,
            last_request: None,
        }
    }

    /// Send a chat completion request and return the assistant's response text.
    pub async fn chat(&mut self, system: &str, user: &str) -> Result<String, LlmError> {
        self.enforce_rate_limit().await;

        let url = format!("{}/v1/chat/completions", self.config.base_url);
        let body = ChatRequest {
            model: self.config.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".into(),
                    content: system.into(),
                },
                ChatMessage {
                    role: "user".into(),
                    content: user.into(),
                },
            ],
            temperature: self.config.temperature,
            max_tokens: self.config.max_tokens,
        };

        let mut last_err = None;
        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                let backoff = Duration::from_secs(1 << (attempt - 1)); // 1s, 2s, 4s
                tokio::time::sleep(backoff).await;
            }

            let result = self.http.post(&url).json(&body).send().await;

            match result {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        self.last_request = Some(Instant::now());
                        let chat_resp: ChatResponse = resp.json().await.map_err(LlmError::Http)?;
                        let content = chat_resp
                            .choices
                            .into_iter()
                            .next()
                            .map(|c| c.message.content)
                            .unwrap_or_default();
                        if content.trim().is_empty() {
                            return Err(LlmError::EmptyResponse);
                        }
                        return Ok(content);
                    }

                    let status_code = status.as_u16();
                    let body_text = resp.text().await.unwrap_or_default();

                    // Retry on 5xx, fail immediately on 4xx
                    if status.is_server_error() {
                        last_err = Some(LlmError::Api {
                            status: status_code,
                            body: body_text,
                        });
                        continue;
                    }
                    return Err(LlmError::Api {
                        status: status_code,
                        body: body_text,
                    });
                }
                Err(e) => {
                    if e.is_timeout() {
                        return Err(LlmError::Timeout(self.config.timeout_secs));
                    }
                    last_err = Some(LlmError::Http(e));
                    continue;
                }
            }
        }

        Err(last_err.unwrap_or(LlmError::EmptyResponse))
    }

    async fn enforce_rate_limit(&self) {
        if let Some(last) = self.last_request {
            let elapsed = last.elapsed();
            if elapsed < RATE_LIMIT_INTERVAL {
                tokio::time::sleep(RATE_LIMIT_INTERVAL - elapsed).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config(base_url: &str) -> LlmConfig {
        LlmConfig {
            base_url: base_url.to_string(),
            model: "test-model".to_string(),
            temperature: 0.3,
            max_tokens: 512,
            timeout_secs: 5,
            max_retries: 1,
            prompt_path: None,
        }
    }

    #[tokio::test]
    async fn chat_success() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/v1/chat/completions")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"choices":[{"message":{"role":"assistant","content":"{\"direction\":\"long\"}"}}]}"#,
            )
            .create_async()
            .await;

        let mut client = LlmClient::new_test(test_config(&server.url()));
        let result = client.chat("system prompt", "user prompt").await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("long"));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn chat_4xx_no_retry() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/v1/chat/completions")
            .with_status(400)
            .with_body("bad request")
            .expect(1)
            .create_async()
            .await;

        let mut client = LlmClient::new_test(test_config(&server.url()));
        let result = client.chat("sys", "usr").await;
        assert!(matches!(result, Err(LlmError::Api { status: 400, .. })));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn chat_5xx_retries() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/v1/chat/completions")
            .with_status(500)
            .with_body("internal error")
            .expect(2) // initial + 1 retry
            .create_async()
            .await;

        let mut client = LlmClient::new_test(test_config(&server.url()));
        let result = client.chat("sys", "usr").await;
        assert!(matches!(result, Err(LlmError::Api { status: 500, .. })));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn chat_empty_response() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/v1/chat/completions")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"choices":[{"message":{"role":"assistant","content":"  "}}]}"#)
            .create_async()
            .await;

        let mut client = LlmClient::new_test(test_config(&server.url()));
        let result = client.chat("sys", "usr").await;
        assert!(matches!(result, Err(LlmError::EmptyResponse)));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn config_serde_defaults() {
        let json = r#"{"base_url":"http://localhost:11434","model":"llama3"}"#;
        let config: LlmConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.temperature, 0.3);
        assert_eq!(config.max_tokens, 512);
        assert_eq!(config.timeout_secs, 30);
        assert_eq!(config.max_retries, 2);
    }
}
