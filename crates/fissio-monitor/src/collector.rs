//! Tracing collector that persists to TraceStore.

use crate::store::TraceStore;
use crate::trace::{SpanRecord, TraceRecord, TraceStatus};
use crate::{MetricsCollector, NodeMetrics, PipelineMetrics};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Collector that persists traces and spans to a TraceStore.
pub struct TracingCollector {
    store: Arc<TraceStore>,
    trace_id: String,
    pipeline_id: String,
    pipeline_name: String,
    input: String,
    start_time: i64,
    metrics: Mutex<Vec<NodeMetrics>>,
    spans: Mutex<Vec<SpanRecord>>,
}

impl TracingCollector {
    /// Creates a new tracing collector and initializes a trace record.
    pub fn new(
        store: Arc<TraceStore>,
        pipeline_id: impl Into<String>,
        pipeline_name: impl Into<String>,
        input: impl Into<String>,
    ) -> Self {
        let trace_id = uuid::Uuid::new_v4().to_string();
        let start_time = now_ms();
        let pipeline_id = pipeline_id.into();
        let pipeline_name = pipeline_name.into();
        let input = input.into();

        // Insert initial trace record (status: running)
        let trace = TraceRecord {
            trace_id: trace_id.clone(),
            pipeline_id: pipeline_id.clone(),
            pipeline_name: pipeline_name.clone(),
            timestamp: start_time,
            input: input.clone(),
            output: String::new(),
            total_elapsed_ms: 0,
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_tool_calls: 0,
            status: TraceStatus::Running,
        };

        if let Err(e) = store.insert_trace(&trace) {
            tracing::warn!("Failed to insert trace: {}", e);
        }

        Self {
            store,
            trace_id,
            pipeline_id,
            pipeline_name,
            input,
            start_time,
            metrics: Mutex::new(Vec::new()),
            spans: Mutex::new(Vec::new()),
        }
    }

    /// Returns the trace ID.
    pub fn trace_id(&self) -> &str {
        &self.trace_id
    }

    /// Finalizes the trace with the given output and status.
    pub fn finalize(&self, output: &str, status: TraceStatus) {
        let elapsed_ms = (now_ms() - self.start_time) as u64;
        let metrics = self.flush();

        let trace = TraceRecord {
            trace_id: self.trace_id.clone(),
            pipeline_id: self.pipeline_id.clone(),
            pipeline_name: self.pipeline_name.clone(),
            timestamp: self.start_time,
            input: self.input.clone(),
            output: output.to_string(),
            total_elapsed_ms: elapsed_ms,
            total_input_tokens: metrics.total_input_tokens,
            total_output_tokens: metrics.total_output_tokens,
            total_tool_calls: metrics.total_tool_calls,
            status,
        };

        if let Err(e) = self.store.update_trace(&trace) {
            tracing::warn!("Failed to update trace: {}", e);
        }
    }

    /// Marks the trace as successful with the given output.
    pub fn success(&self, output: &str) {
        self.finalize(output, TraceStatus::Success);
    }

    /// Marks the trace as failed with the given error message.
    pub fn error(&self, error: &str) {
        self.finalize(error, TraceStatus::Error);
    }
}

impl MetricsCollector for TracingCollector {
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

    fn record_span(
        &self,
        node_id: &str,
        node_type: &str,
        start_time: i64,
        end_time: i64,
        input: &str,
        output: &str,
        metrics: &NodeMetrics,
    ) {
        let span = SpanRecord {
            span_id: uuid::Uuid::new_v4().to_string(),
            trace_id: self.trace_id.clone(),
            node_id: node_id.to_string(),
            node_type: node_type.to_string(),
            start_time,
            end_time,
            input: input.to_string(),
            output: output.to_string(),
            input_tokens: metrics.input_tokens,
            output_tokens: metrics.output_tokens,
            tool_call_count: metrics.tool_call_count,
            iteration_count: metrics.iteration_count,
        };

        if let Err(e) = self.store.insert_span(&span) {
            tracing::warn!("Failed to insert span: {}", e);
        }

        let Ok(mut spans) = self.spans.lock() else { return };
        spans.push(span);
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
        let Ok(mut guard) = self.metrics.lock() else { return };
        guard.clear();
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracing_collector() {
        let store = Arc::new(TraceStore::in_memory().unwrap());
        let collector = TracingCollector::new(
            store.clone(),
            "test-pipe",
            "Test Pipeline",
            "Hello",
        );

        collector.record(NodeMetrics {
            node_id: "node1".to_string(),
            input_tokens: 10,
            output_tokens: 20,
            elapsed_ms: 100,
            tool_call_count: 1,
            iteration_count: 1,
            estimated_cost_usd: None,
        });

        collector.success("World");

        let trace = store.get_trace(collector.trace_id()).unwrap().unwrap();
        assert_eq!(trace.status, TraceStatus::Success);
        assert_eq!(trace.output, "World");
        assert_eq!(trace.total_input_tokens, 10);
        assert_eq!(trace.total_output_tokens, 20);
    }
}
