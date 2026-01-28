//! Observability and metrics collection for fissio pipelines.

mod collector;
mod store;
mod trace;

pub use collector::TracingCollector;
pub use store::{MetricsSummary, StoreError, TraceStore};
pub use trace::{SpanRecord, ToolCallRecord, TraceQuery, TraceRecord, TraceStatus};

use serde::{Deserialize, Serialize};
use std::sync::Mutex;

/// Configuration for per-node observability.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ObserveConfig {
    /// Enable metrics collection for this node.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Include token usage metrics.
    #[serde(default = "default_true")]
    pub tokens: bool,
    /// Include latency metrics.
    #[serde(default = "default_true")]
    pub latency: bool,
    /// Include tool call counts.
    #[serde(default = "default_true")]
    pub tool_calls: bool,
    /// Include cost estimation (requires model pricing).
    #[serde(default)]
    pub cost: bool,
}

fn default_true() -> bool {
    true
}

impl ObserveConfig {
    /// Creates a new ObserveConfig with all metrics enabled.
    pub fn new() -> Self {
        Self {
            enabled: true,
            tokens: true,
            latency: true,
            tool_calls: true,
            cost: false,
        }
    }

    /// Creates a config with only specified metrics enabled.
    pub fn with_tokens(mut self, enabled: bool) -> Self {
        self.tokens = enabled;
        self
    }

    pub fn with_latency(mut self, enabled: bool) -> Self {
        self.latency = enabled;
        self
    }

    pub fn with_tool_calls(mut self, enabled: bool) -> Self {
        self.tool_calls = enabled;
        self
    }

    pub fn with_cost(mut self, enabled: bool) -> Self {
        self.cost = enabled;
        self
    }
}

/// Metrics collected from a single node execution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeMetrics {
    /// Node identifier.
    pub node_id: String,
    /// Input tokens consumed.
    pub input_tokens: u32,
    /// Output tokens generated.
    pub output_tokens: u32,
    /// Total elapsed time in milliseconds.
    pub elapsed_ms: u64,
    /// Number of tool calls made.
    pub tool_call_count: u32,
    /// Number of agentic loop iterations.
    pub iteration_count: u32,
    /// Estimated cost in USD (if pricing configured).
    pub estimated_cost_usd: Option<f64>,
}

impl NodeMetrics {
    pub fn new(node_id: impl Into<String>) -> Self {
        Self {
            node_id: node_id.into(),
            ..Default::default()
        }
    }

    pub fn total_tokens(&self) -> u32 {
        self.input_tokens + self.output_tokens
    }
}

/// Aggregated metrics for a pipeline execution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PipelineMetrics {
    /// Pipeline identifier.
    pub pipeline_id: String,
    /// Total input tokens across all nodes.
    pub total_input_tokens: u32,
    /// Total output tokens across all nodes.
    pub total_output_tokens: u32,
    /// Total elapsed time in milliseconds.
    pub total_elapsed_ms: u64,
    /// Total tool calls across all nodes.
    pub total_tool_calls: u32,
    /// Per-node metrics.
    pub node_metrics: Vec<NodeMetrics>,
}

impl PipelineMetrics {
    pub fn total_tokens(&self) -> u32 {
        self.total_input_tokens + self.total_output_tokens
    }

    pub fn total_cost(&self) -> f64 {
        self.node_metrics
            .iter()
            .filter_map(|m| m.estimated_cost_usd)
            .sum()
    }
}

/// Trait for metrics collectors.
pub trait MetricsCollector: Send + Sync {
    /// Record metrics from a node execution.
    fn record(&self, metrics: NodeMetrics);
    /// Record a span with node I/O for detailed tracing.
    fn record_span(
        &self,
        _node_id: &str,
        _node_type: &str,
        _start_time: i64,
        _end_time: i64,
        _input: &str,
        _output: &str,
        _metrics: &NodeMetrics,
    ) {
        // Default no-op - override in TracingCollector
    }
    /// Flush and return aggregated pipeline metrics.
    fn flush(&self) -> PipelineMetrics;
    /// Reset the collector for a new pipeline run.
    fn reset(&self);
}

/// In-memory metrics collector (default implementation).
pub struct InMemoryCollector {
    pipeline_id: String,
    metrics: Mutex<Vec<NodeMetrics>>,
}

impl InMemoryCollector {
    pub fn new(pipeline_id: impl Into<String>) -> Self {
        Self {
            pipeline_id: pipeline_id.into(),
            metrics: Mutex::new(Vec::new()),
        }
    }
}

impl MetricsCollector for InMemoryCollector {
    fn record(&self, metrics: NodeMetrics) {
        let Ok(mut guard) = self.metrics.lock() else {
            tracing::warn!("Failed to acquire metrics lock");
            return;
        };
        tracing::debug!(
            node_id = %metrics.node_id,
            input_tokens = metrics.input_tokens,
            output_tokens = metrics.output_tokens,
            elapsed_ms = metrics.elapsed_ms,
            "Recorded node metrics"
        );
        guard.push(metrics);
    }

    fn flush(&self) -> PipelineMetrics {
        let Ok(guard) = self.metrics.lock() else {
            return PipelineMetrics {
                pipeline_id: self.pipeline_id.clone(),
                ..Default::default()
            };
        };

        let mut pm = PipelineMetrics {
            pipeline_id: self.pipeline_id.clone(),
            node_metrics: guard.clone(),
            ..Default::default()
        };

        for m in &pm.node_metrics {
            pm.total_input_tokens += m.input_tokens;
            pm.total_output_tokens += m.output_tokens;
            pm.total_elapsed_ms += m.elapsed_ms;
            pm.total_tool_calls += m.tool_call_count;
        }

        pm
    }

    fn reset(&self) {
        let Ok(mut guard) = self.metrics.lock() else {
            return;
        };
        guard.clear();
    }
}

/// Model pricing for cost estimation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricing {
    /// Cost per 1K input tokens in USD.
    pub input_per_1k: f64,
    /// Cost per 1K output tokens in USD.
    pub output_per_1k: f64,
}

impl ModelPricing {
    pub fn new(input_per_1k: f64, output_per_1k: f64) -> Self {
        Self {
            input_per_1k,
            output_per_1k,
        }
    }

    /// Estimate cost for given token counts.
    pub fn estimate(&self, input_tokens: u32, output_tokens: u32) -> f64 {
        (input_tokens as f64 / 1000.0) * self.input_per_1k
            + (output_tokens as f64 / 1000.0) * self.output_per_1k
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_observe_config_defaults() {
        let config = ObserveConfig::new();
        assert!(config.enabled);
        assert!(config.tokens);
        assert!(config.latency);
        assert!(config.tool_calls);
        assert!(!config.cost);
    }

    #[test]
    fn test_in_memory_collector() {
        let collector = InMemoryCollector::new("test-pipeline");

        collector.record(NodeMetrics {
            node_id: "node1".to_string(),
            input_tokens: 100,
            output_tokens: 50,
            elapsed_ms: 200,
            tool_call_count: 2,
            iteration_count: 1,
            estimated_cost_usd: None,
        });

        collector.record(NodeMetrics {
            node_id: "node2".to_string(),
            input_tokens: 150,
            output_tokens: 75,
            elapsed_ms: 300,
            tool_call_count: 0,
            iteration_count: 1,
            estimated_cost_usd: None,
        });

        let metrics = collector.flush();
        assert_eq!(metrics.pipeline_id, "test-pipeline");
        assert_eq!(metrics.total_input_tokens, 250);
        assert_eq!(metrics.total_output_tokens, 125);
        assert_eq!(metrics.total_elapsed_ms, 500);
        assert_eq!(metrics.total_tool_calls, 2);
        assert_eq!(metrics.node_metrics.len(), 2);
    }

    #[test]
    fn test_model_pricing() {
        let pricing = ModelPricing::new(0.01, 0.03);
        let cost = pricing.estimate(1000, 500);
        assert!((cost - 0.025).abs() < 0.0001);
    }
}
