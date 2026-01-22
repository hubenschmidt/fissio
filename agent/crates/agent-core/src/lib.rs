use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ─────────────────────────────────────────────────────────────────────────────
// Error
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Error, Debug)]
pub enum AgentError {
    #[error("LLM request failed: {0}")]
    LlmError(String),

    #[error("Failed to parse structured output: {0}")]
    ParseError(String),

    #[error("Worker execution failed: {0}")]
    WorkerFailed(String),

    #[error("External API error: {0}")]
    ExternalApi(String),

    #[error("Max retries exceeded")]
    MaxRetriesExceeded,

    #[error("Unknown worker type: {0}")]
    UnknownWorker(String),

    #[error("WebSocket error: {0}")]
    WebSocket(String),
}

impl From<serde_json::Error> for AgentError {
    fn from(err: serde_json::Error) -> Self {
        AgentError::ParseError(err.to_string())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum WorkerType {
    Search,
    Email,
    General,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Handoff {
    pub target: WorkerType,
    pub context: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorDecision {
    pub worker_type: WorkerType,
    pub task_description: String,
    pub parameters: serde_json::Value,
    pub success_criteria: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatorResult {
    pub passed: bool,
    pub score: u8,
    pub feedback: String,
    #[serde(default)]
    pub suggestions: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerResult {
    pub success: bool,
    pub output: String,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub handoff: Option<Handoff>,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailParams {
    pub to: String,
    pub subject: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchParams {
    pub query: String,
    #[serde(default = "default_num_results")]
    pub num_results: u8,
}

fn default_num_results() -> u8 {
    5
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontlineDecision {
    pub should_route: bool,
    pub response: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub id: String,
    pub name: String,
    pub model: String,
    pub api_base: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Worker Trait
// ─────────────────────────────────────────────────────────────────────────────

#[async_trait]
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
