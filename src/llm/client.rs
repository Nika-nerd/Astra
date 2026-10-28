use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone)]
pub struct LlmClient {
    http: Client,
    api_url: String,
    model: String,
}

#[derive(Serialize)]
struct OllamaRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    system: &'a str,
    stream: bool,
    options: OllamaOptions,
}

#[derive(Serialize)]
struct OllamaOptions {
    temperature: f32,
}

#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
}

impl LlmClient {
    pub fn new(api_url: String, model: String) -> Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(300)) // Локальная генерация может занимать время
            .build()
            .context("Failed to build HTTP client")?;

        Ok(Self {
            http,
            api_url,
            model,
        })
    }

    pub async fn generate(&self, system_prompt: &str, user_prompt: &str) -> Result<String> {
        let req_body = OllamaRequest {
            model: &self.model,
            prompt: user_prompt,
            system: system_prompt,
            stream: false,
            options: OllamaOptions {
                temperature: 0.1, // Низкая температура для детерминированной генерации кода
            },
        };

        let resp = self
            .http
            .post(&self.api_url)
            .json(&req_body)
            .send()
            .await
            .with_context(|| format!("Failed to send request to LLM at {}", self.api_url))?
            .error_for_status()
            .context("LLM server returned an error status")?;

        let parsed: OllamaResponse = resp
            .json()
            .await
            .context("Failed to parse JSON response from LLM")?;

        Ok(parsed.response)
    }
}