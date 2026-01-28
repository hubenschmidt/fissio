//! Trace storage types for observability.

use serde::{Deserialize, Serialize};

/// A complete execution trace for a pipeline run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceRecord {
    /// Unique trace identifier.
    pub trace_id: String,
    /// Pipeline identifier.
    pub pipeline_id: String,
    /// Pipeline name for display.
    pub pipeline_name: String,
    /// Unix timestamp (milliseconds) when trace started.
    pub timestamp: i64,
    /// User input that started the pipeline.
    pub input: String,
    /// Final output from the pipeline.
    pub output: String,
    /// Total execution time in milliseconds.
    pub total_elapsed_ms: u64,
    /// Total input tokens across all spans.
    pub total_input_tokens: u32,
    /// Total output tokens across all spans.
    pub total_output_tokens: u32,
    /// Total tool calls across all spans.
    pub total_tool_calls: u32,
    /// Execution status.
    pub status: TraceStatus,
}

/// Status of a trace execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TraceStatus {
    /// Execution completed successfully.
    Success,
    /// Execution failed with an error.
    Error,
    /// Execution is still in progress.
    Running,
}

impl TraceStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TraceStatus::Success => "success",
            TraceStatus::Error => "error",
            TraceStatus::Running => "running",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "success" => TraceStatus::Success,
            "error" => TraceStatus::Error,
            "running" => TraceStatus::Running,
            _ => TraceStatus::Error,
        }
    }
}

/// A span representing a single node execution within a trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanRecord {
    /// Unique span identifier.
    pub span_id: String,
    /// Parent trace identifier.
    pub trace_id: String,
    /// Node identifier.
    pub node_id: String,
    /// Node type (e.g., "llm", "worker", "router").
    pub node_type: String,
    /// Unix timestamp (milliseconds) when span started.
    pub start_time: i64,
    /// Unix timestamp (milliseconds) when span ended.
    pub end_time: i64,
    /// Input to this node.
    pub input: String,
    /// Output from this node.
    pub output: String,
    /// Input tokens for this span.
    pub input_tokens: u32,
    /// Output tokens for this span.
    pub output_tokens: u32,
    /// Number of tool calls in this span.
    pub tool_call_count: u32,
    /// Number of agentic loop iterations.
    pub iteration_count: u32,
}

/// A tool call record within a span.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    /// Unique tool call identifier.
    pub call_id: String,
    /// Parent span identifier.
    pub span_id: String,
    /// Tool name.
    pub tool_name: String,
    /// Tool arguments as JSON.
    pub arguments: serde_json::Value,
    /// Tool result.
    pub result: String,
    /// Execution time in milliseconds.
    pub elapsed_ms: u64,
}

/// Query parameters for listing traces.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TraceQuery {
    /// Filter by pipeline ID.
    pub pipeline_id: Option<String>,
    /// Filter by status.
    pub status: Option<TraceStatus>,
    /// Maximum number of traces to return.
    pub limit: Option<u32>,
    /// Offset for pagination.
    pub offset: Option<u32>,
}
