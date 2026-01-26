//! Anthropic Claude API client with streaming support.

use agent_core::{AgentError, Message, MessageRole};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::{LlmMetrics, LlmResponse, LlmStream, StreamChunk};

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

#[derive(Serialize)]
struct AnthropicMessage {
    role: &'static str,
    content: String,
}

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<AnthropicMessage>,
    stream: bool,
}

#[derive(Deserialize)]
struct ContentBlockDelta {
    text: Option<String>,
}

#[derive(Deserialize)]
struct Usage {
    input_tokens: Option<u32>,
    output_tokens: Option<u32>,
}

#[derive(Deserialize)]
struct StreamEvent {
    #[serde(rename = "type")]
    event_type: String,
    delta: Option<ContentBlockDelta>,
    usage: Option<Usage>,
    message: Option<MessageEvent>,
}

#[derive(Deserialize)]
struct MessageEvent {
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct ContentBlock {
    text: String,
}

#[derive(Deserialize)]
struct NonStreamResponse {
    content: Vec<ContentBlock>,
    usage: Usage,
}

/// Client for Anthropic's Claude API.
pub struct AnthropicClient {
    client: Client,
    model: String,
    api_key: String,
}

impl AnthropicClient {
    /// Creates a new Anthropic client.
    pub fn new(model: &str) -> Self {
        let api_key = std::env::var("ANTHROPIC_API_KEY").unwrap_or_default();
        tracing::info!(
            "AnthropicClient: model={}, api_key_len={}",
            model,
            api_key.len()
        );
        Self {
            client: Client::new(),
            model: model.to_string(),
            api_key,
        }
    }

    /// Sends a non-streaming chat request and returns the complete response.
    pub async fn chat(&self, system_prompt: &str, user_input: &str) -> Result<LlmResponse, AgentError> {
        let start = std::time::Instant::now();

        let request = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: 8192,
            system: system_prompt.to_string(),
            messages: vec![AnthropicMessage {
                role: "user",
                content: user_input.to_string(),
            }],
            stream: false,
        };

        let response = self
            .client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| AgentError::LlmError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AgentError::LlmError(format!(
                "Anthropic API error {}: {}",
                status, body
            )));
        }

        let resp: NonStreamResponse = response
            .json()
            .await
            .map_err(|e| AgentError::LlmError(e.to_string()))?;

        let content = resp.content.into_iter().map(|c| c.text).collect::<Vec<_>>().join("");

        Ok(LlmResponse {
            content,
            metrics: LlmMetrics {
                input_tokens: resp.usage.input_tokens.unwrap_or(0),
                output_tokens: resp.usage.output_tokens.unwrap_or(0),
                elapsed_ms: start.elapsed().as_millis() as u64,
            },
        })
    }

    /// Sends a chat request with history and returns a stream of chunks.
    pub async fn chat_stream(
        &self,
        system_prompt: &str,
        history: &[Message],
        user_input: &str,
    ) -> Result<LlmStream, AgentError> {
        use futures::StreamExt;

        let mut messages: Vec<AnthropicMessage> = history
            .iter()
            .map(|msg| AnthropicMessage {
                role: match msg.role {
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                },
                content: msg.content.clone(),
            })
            .collect();

        messages.push(AnthropicMessage {
            role: "user",
            content: user_input.to_string(),
        });

        let request = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: 8192,
            system: system_prompt.to_string(),
            messages,
            stream: true,
        };

        let response = self
            .client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| AgentError::LlmError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AgentError::LlmError(format!(
                "Anthropic API error {}: {}",
                status, body
            )));
        }

        let byte_stream = response.bytes_stream();

        // Use scan to maintain a buffer across chunks for incomplete SSE lines
        let mapped = byte_stream
            .scan(String::new(), |buffer, result| {
                let chunks: Vec<Result<StreamChunk, AgentError>> = match result {
                    Err(e) => vec![Err(AgentError::LlmError(e.to_string()))],
                    Ok(bytes) => {
                        let text = match String::from_utf8(bytes.to_vec()) {
                            Ok(t) => t,
                            Err(_) => return futures::future::ready(Some(vec![])),
                        };

                        buffer.push_str(&text);

                        let mut parsed_chunks = Vec::new();

                        // Process complete lines, keep incomplete line in buffer
                        while let Some(newline_pos) = buffer.find('\n') {
                            let line = buffer[..newline_pos].trim().to_string();
                            *buffer = buffer[newline_pos + 1..].to_string();

                            if !line.starts_with("data: ") {
                                continue;
                            }
                            let json = &line[6..];
                            if json == "[DONE]" {
                                continue;
                            }

                            let event: StreamEvent = match serde_json::from_str(json) {
                                Ok(e) => e,
                                Err(e) => {
                                    error!("Failed to parse Anthropic event: {} - {}", e, json);
                                    continue;
                                }
                            };

                            match event.event_type.as_str() {
                                "content_block_delta" => {
                                    if let Some(delta) = event.delta {
                                        if let Some(text) = delta.text {
                                            parsed_chunks.push(Ok(StreamChunk::Content(text)));
                                        }
                                    }
                                }
                                "message_delta" => {
                                    if let Some(usage) = event.usage {
                                        parsed_chunks.push(Ok(StreamChunk::Usage {
                                            input_tokens: usage.input_tokens.unwrap_or(0),
                                            output_tokens: usage.output_tokens.unwrap_or(0),
                                        }));
                                    }
                                }
                                "message_start" => {
                                    if let Some(msg) = event.message {
                                        if let Some(usage) = msg.usage {
                                            parsed_chunks.push(Ok(StreamChunk::Usage {
                                                input_tokens: usage.input_tokens.unwrap_or(0),
                                                output_tokens: 0,
                                            }));
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                        parsed_chunks
                    }
                };
                futures::future::ready(Some(chunks))
            })
            .flat_map(futures::stream::iter);

        Ok(Box::pin(mapped))
    }
}
