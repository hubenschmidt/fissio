//! Core domain types and error definitions for fissio.
//!
//! This crate provides the fundamental types shared across the fissio framework:
//!
//! - [`AgentError`] — Error type for pipeline and LLM operations
//! - [`Message`] and [`MessageRole`] — Conversation message types
//! - [`ModelConfig`] — LLM model configuration
//!
//! # Example
//!
//! ```rust
//! use fissio_core::{Message, MessageRole, ModelConfig};
//!
//! let msg = Message {
//!     role: MessageRole::User,
//!     content: "Hello!".to_string(),
//! };
//!
//! let model = ModelConfig {
//!     id: "gpt-4".to_string(),
//!     name: "GPT-4".to_string(),
//!     model: "gpt-4-turbo".to_string(),
//!     api_base: None,
//! };
//! ```

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur during pipeline execution or LLM operations.
#[derive(Error, Debug)]
pub enum AgentError {
    /// LLM API request failed.
    #[error("LLM request failed: {0}")]
    LlmError(String),

    /// Failed to parse structured output from LLM.
    #[error("Failed to parse structured output: {0}")]
    ParseError(String),

    /// Worker node execution failed.
    #[error("Worker execution failed: {0}")]
    WorkerFailed(String),

    /// External API call failed.
    #[error("External API error: {0}")]
    ExternalApi(String),

    /// Maximum retry attempts exceeded.
    #[error("Max retries exceeded")]
    MaxRetriesExceeded,

    /// Unknown worker type specified.
    #[error("Unknown worker type: {0}")]
    UnknownWorker(String),

    /// WebSocket communication error.
    #[error("WebSocket error: {0}")]
    WebSocket(String),
}

impl From<serde_json::Error> for AgentError {
    fn from(err: serde_json::Error) -> Self {
        AgentError::ParseError(err.to_string())
    }
}

/// Role of a message in a conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// Message from the user.
    User,
    /// Message from the assistant/LLM.
    Assistant,
}

/// A single message in a conversation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// The role of the message sender.
    pub role: MessageRole,
    /// The content of the message.
    pub content: String,
}

impl Message {
    /// Creates a new user message.
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: MessageRole::User, content: content.into() }
    }

    /// Creates a new assistant message.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: MessageRole::Assistant, content: content.into() }
    }
}

/// Configuration for an LLM model.
///
/// Used to specify which model to use for pipeline nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// Unique identifier for this model configuration.
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// The actual model identifier (e.g., "gpt-4-turbo", "claude-3-opus").
    pub model: String,
    /// Optional API base URL for self-hosted or alternative endpoints.
    pub api_base: Option<String>,
}

// ============================================================================
// Legacy types - kept for backwards compatibility but hidden from docs
// ============================================================================

/// Types of workers that can execute tasks.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum WorkerType {
    Search,
    Email,
    General,
}

/// A handoff request to transfer work to another worker.
#[doc(hidden)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Handoff {
    pub target: WorkerType,
    pub context: String,
}

/// Decision made by the orchestrator about which worker to use.
#[doc(hidden)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorDecision {
    pub worker_type: WorkerType,
    pub task_description: String,
    pub parameters: serde_json::Value,
    pub success_criteria: String,
}

/// Result of an evaluator checking work quality.
#[doc(hidden)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatorResult {
    pub passed: bool,
    pub score: u8,
    pub feedback: String,
    #[serde(default)]
    pub suggestions: String,
}

/// Result returned by a worker after execution.
#[doc(hidden)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerResult {
    pub success: bool,
    pub output: String,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub handoff: Option<Handoff>,
}

#[doc(hidden)]
impl WorkerResult {
    pub fn ok(output: String) -> Self {
        Self { success: true, output, error: None, handoff: None }
    }

    pub fn err(e: impl ToString) -> Self {
        Self { success: false, output: String::new(), error: Some(e.to_string()), handoff: None }
    }

    pub fn handoff(target: WorkerType, context: String) -> Self {
        Self {
            success: true,
            output: String::new(),
            error: None,
            handoff: Some(Handoff { target, context }),
        }
    }
}

/// Parameters for sending an email.
#[doc(hidden)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailParams {
    pub to: String,
    pub subject: String,
    pub body: String,
}

/// Parameters for a search query.
#[doc(hidden)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchParams {
    pub query: String,
    #[serde(default = "default_num_results")]
    pub num_results: u8,
}

#[doc(hidden)]
fn default_num_results() -> u8 {
    5
}

/// Decision made by the frontline agent about routing.
#[doc(hidden)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontlineDecision {
    pub should_route: bool,
    pub response: String,
}

/// Trait for workers that can execute tasks.
#[doc(hidden)]
#[async_trait::async_trait]
pub trait Worker: Send + Sync {
    fn worker_type(&self) -> WorkerType;
    async fn execute(
        &self,
        task_description: &str,
        parameters: &serde_json::Value,
        feedback: Option<&str>,
        model: &ModelConfig,
    ) -> Result<WorkerResult, AgentError>;
}
