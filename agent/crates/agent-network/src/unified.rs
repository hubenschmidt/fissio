//! Unified LLM client that routes to the appropriate provider based on model name.

use agent_core::{AgentError, Message};

use crate::anthropic::AnthropicClient;
use crate::client::LlmClient;
use crate::{LlmResponse, LlmStream};

/// Provider type determined from model name.
#[derive(Debug, Clone, Copy)]
enum ProviderType {
    OpenAI,
    Anthropic,
}

/// Unified client that routes requests to OpenAI or Anthropic based on model name.
pub struct UnifiedLlmClient {
    model: String,
    provider: ProviderType,
    api_base: Option<String>,
}

impl UnifiedLlmClient {
    /// Creates a new unified client, detecting provider from model name.
    pub fn new(model: &str, api_base: Option<&str>) -> Self {
        let provider = match model.starts_with("claude-") {
            true => ProviderType::Anthropic,
            false => ProviderType::OpenAI,
        };

        Self {
            model: model.to_string(),
            provider,
            api_base: api_base.map(String::from),
        }
    }

    /// Returns true if this client is configured for Anthropic.
    pub fn is_anthropic(&self) -> bool {
        matches!(self.provider, ProviderType::Anthropic)
    }

    /// Sends a non-streaming chat request and returns the complete response.
    pub async fn chat(&self, system_prompt: &str, user_input: &str) -> Result<LlmResponse, AgentError> {
        match self.provider {
            ProviderType::OpenAI => {
                let client = LlmClient::new(&self.model, self.api_base.as_deref());
                client.chat(system_prompt, user_input).await
            }
            ProviderType::Anthropic => {
                let client = AnthropicClient::new(&self.model);
                client.chat(system_prompt, user_input).await
            }
        }
    }

    /// Sends a chat request with history and returns a stream of chunks.
    pub async fn chat_stream(
        &self,
        system_prompt: &str,
        history: &[Message],
        user_input: &str,
    ) -> Result<LlmStream, AgentError> {
        match self.provider {
            ProviderType::OpenAI => {
                let client = LlmClient::new(&self.model, self.api_base.as_deref());
                client.chat_stream(system_prompt, history, user_input).await
            }
            ProviderType::Anthropic => {
                let client = AnthropicClient::new(&self.model);
                client.chat_stream(system_prompt, history, user_input).await
            }
        }
    }
}
