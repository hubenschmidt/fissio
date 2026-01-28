//! SQLite-backed trace storage.

use crate::trace::{SpanRecord, ToolCallRecord, TraceQuery, TraceRecord, TraceStatus};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;
use thiserror::Error;

/// Errors from trace store operations.
#[derive(Debug, Error)]
pub enum StoreError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("Lock error")]
    Lock,
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// SQLite-backed trace storage.
pub struct TraceStore {
    conn: Mutex<Connection>,
}

impl TraceStore {
    /// Creates a new trace store with the given database path.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let conn = Connection::open(path)?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_schema()?;
        Ok(store)
    }

    /// Creates an in-memory trace store (for testing).
    pub fn in_memory() -> Result<Self, StoreError> {
        let conn = Connection::open_in_memory()?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<(), StoreError> {
        let conn = self.conn.lock().map_err(|_| StoreError::Lock)?;

        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS traces (
                trace_id TEXT PRIMARY KEY,
                pipeline_id TEXT NOT NULL,
                pipeline_name TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                input TEXT NOT NULL,
                output TEXT NOT NULL,
                total_elapsed_ms INTEGER NOT NULL,
                total_input_tokens INTEGER NOT NULL,
                total_output_tokens INTEGER NOT NULL,
                total_tool_calls INTEGER NOT NULL,
                status TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS spans (
                span_id TEXT PRIMARY KEY,
                trace_id TEXT NOT NULL,
                node_id TEXT NOT NULL,
                node_type TEXT NOT NULL,
                start_time INTEGER NOT NULL,
                end_time INTEGER NOT NULL,
                input TEXT NOT NULL,
                output TEXT NOT NULL,
                input_tokens INTEGER NOT NULL,
                output_tokens INTEGER NOT NULL,
                tool_call_count INTEGER NOT NULL,
                iteration_count INTEGER NOT NULL,
                FOREIGN KEY (trace_id) REFERENCES traces(trace_id)
            );

            CREATE TABLE IF NOT EXISTS tool_calls (
                call_id TEXT PRIMARY KEY,
                span_id TEXT NOT NULL,
                tool_name TEXT NOT NULL,
                arguments TEXT NOT NULL,
                result TEXT NOT NULL,
                elapsed_ms INTEGER NOT NULL,
                FOREIGN KEY (span_id) REFERENCES spans(span_id)
            );

            CREATE INDEX IF NOT EXISTS idx_traces_timestamp ON traces(timestamp DESC);
            CREATE INDEX IF NOT EXISTS idx_traces_pipeline ON traces(pipeline_id);
            CREATE INDEX IF NOT EXISTS idx_spans_trace ON spans(trace_id);
            CREATE INDEX IF NOT EXISTS idx_tool_calls_span ON tool_calls(span_id);
            "#,
        )?;

        Ok(())
    }

    /// Inserts a new trace record.
    pub fn insert_trace(&self, trace: &TraceRecord) -> Result<(), StoreError> {
        let conn = self.conn.lock().map_err(|_| StoreError::Lock)?;

        conn.execute(
            r#"INSERT INTO traces
               (trace_id, pipeline_id, pipeline_name, timestamp, input, output,
                total_elapsed_ms, total_input_tokens, total_output_tokens,
                total_tool_calls, status)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"#,
            params![
                trace.trace_id,
                trace.pipeline_id,
                trace.pipeline_name,
                trace.timestamp,
                trace.input,
                trace.output,
                trace.total_elapsed_ms,
                trace.total_input_tokens,
                trace.total_output_tokens,
                trace.total_tool_calls,
                trace.status.as_str(),
            ],
        )?;

        Ok(())
    }

    /// Updates an existing trace record.
    pub fn update_trace(&self, trace: &TraceRecord) -> Result<(), StoreError> {
        let conn = self.conn.lock().map_err(|_| StoreError::Lock)?;

        conn.execute(
            r#"UPDATE traces SET
               output = ?1, total_elapsed_ms = ?2, total_input_tokens = ?3,
               total_output_tokens = ?4, total_tool_calls = ?5, status = ?6
               WHERE trace_id = ?7"#,
            params![
                trace.output,
                trace.total_elapsed_ms,
                trace.total_input_tokens,
                trace.total_output_tokens,
                trace.total_tool_calls,
                trace.status.as_str(),
                trace.trace_id,
            ],
        )?;

        Ok(())
    }

    /// Retrieves a trace by ID.
    pub fn get_trace(&self, trace_id: &str) -> Result<Option<TraceRecord>, StoreError> {
        let conn = self.conn.lock().map_err(|_| StoreError::Lock)?;

        let mut stmt = conn.prepare(
            r#"SELECT trace_id, pipeline_id, pipeline_name, timestamp, input, output,
               total_elapsed_ms, total_input_tokens, total_output_tokens,
               total_tool_calls, status
               FROM traces WHERE trace_id = ?1"#,
        )?;

        let result = stmt.query_row(params![trace_id], |row| {
            Ok(TraceRecord {
                trace_id: row.get(0)?,
                pipeline_id: row.get(1)?,
                pipeline_name: row.get(2)?,
                timestamp: row.get(3)?,
                input: row.get(4)?,
                output: row.get(5)?,
                total_elapsed_ms: row.get(6)?,
                total_input_tokens: row.get(7)?,
                total_output_tokens: row.get(8)?,
                total_tool_calls: row.get(9)?,
                status: TraceStatus::from_str(&row.get::<_, String>(10)?),
            })
        });

        match result {
            Ok(trace) => Ok(Some(trace)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Lists traces with optional filtering.
    pub fn list_traces(&self, query: &TraceQuery) -> Result<Vec<TraceRecord>, StoreError> {
        let conn = self.conn.lock().map_err(|_| StoreError::Lock)?;

        let mut sql = String::from(
            r#"SELECT trace_id, pipeline_id, pipeline_name, timestamp, input, output,
               total_elapsed_ms, total_input_tokens, total_output_tokens,
               total_tool_calls, status
               FROM traces WHERE 1=1"#,
        );

        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ref pid) = query.pipeline_id {
            sql.push_str(" AND pipeline_id = ?");
            params_vec.push(Box::new(pid.clone()));
        }

        if let Some(status) = query.status {
            sql.push_str(" AND status = ?");
            params_vec.push(Box::new(status.as_str().to_string()));
        }

        sql.push_str(" ORDER BY timestamp DESC");

        if let Some(limit) = query.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        if let Some(offset) = query.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let mut stmt = conn.prepare(&sql)?;

        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();

        let rows = stmt.query_map(params_refs.as_slice(), |row| {
            Ok(TraceRecord {
                trace_id: row.get(0)?,
                pipeline_id: row.get(1)?,
                pipeline_name: row.get(2)?,
                timestamp: row.get(3)?,
                input: row.get(4)?,
                output: row.get(5)?,
                total_elapsed_ms: row.get(6)?,
                total_input_tokens: row.get(7)?,
                total_output_tokens: row.get(8)?,
                total_tool_calls: row.get(9)?,
                status: TraceStatus::from_str(&row.get::<_, String>(10)?),
            })
        })?;

        let mut traces = Vec::new();
        for row in rows {
            traces.push(row?);
        }

        Ok(traces)
    }

    /// Inserts a span record.
    pub fn insert_span(&self, span: &SpanRecord) -> Result<(), StoreError> {
        let conn = self.conn.lock().map_err(|_| StoreError::Lock)?;

        conn.execute(
            r#"INSERT INTO spans
               (span_id, trace_id, node_id, node_type, start_time, end_time,
                input, output, input_tokens, output_tokens, tool_call_count, iteration_count)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)"#,
            params![
                span.span_id,
                span.trace_id,
                span.node_id,
                span.node_type,
                span.start_time,
                span.end_time,
                span.input,
                span.output,
                span.input_tokens,
                span.output_tokens,
                span.tool_call_count,
                span.iteration_count,
            ],
        )?;

        Ok(())
    }

    /// Gets all spans for a trace.
    pub fn get_spans(&self, trace_id: &str) -> Result<Vec<SpanRecord>, StoreError> {
        let conn = self.conn.lock().map_err(|_| StoreError::Lock)?;

        let mut stmt = conn.prepare(
            r#"SELECT span_id, trace_id, node_id, node_type, start_time, end_time,
               input, output, input_tokens, output_tokens, tool_call_count, iteration_count
               FROM spans WHERE trace_id = ?1 ORDER BY start_time"#,
        )?;

        let rows = stmt.query_map(params![trace_id], |row| {
            Ok(SpanRecord {
                span_id: row.get(0)?,
                trace_id: row.get(1)?,
                node_id: row.get(2)?,
                node_type: row.get(3)?,
                start_time: row.get(4)?,
                end_time: row.get(5)?,
                input: row.get(6)?,
                output: row.get(7)?,
                input_tokens: row.get(8)?,
                output_tokens: row.get(9)?,
                tool_call_count: row.get(10)?,
                iteration_count: row.get(11)?,
            })
        })?;

        let mut spans = Vec::new();
        for row in rows {
            spans.push(row?);
        }

        Ok(spans)
    }

    /// Inserts a tool call record.
    pub fn insert_tool_call(&self, call: &ToolCallRecord) -> Result<(), StoreError> {
        let conn = self.conn.lock().map_err(|_| StoreError::Lock)?;

        conn.execute(
            r#"INSERT INTO tool_calls (call_id, span_id, tool_name, arguments, result, elapsed_ms)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6)"#,
            params![
                call.call_id,
                call.span_id,
                call.tool_name,
                serde_json::to_string(&call.arguments)?,
                call.result,
                call.elapsed_ms,
            ],
        )?;

        Ok(())
    }

    /// Gets all tool calls for a span.
    pub fn get_tool_calls(&self, span_id: &str) -> Result<Vec<ToolCallRecord>, StoreError> {
        let conn = self.conn.lock().map_err(|_| StoreError::Lock)?;

        let mut stmt = conn.prepare(
            r#"SELECT call_id, span_id, tool_name, arguments, result, elapsed_ms
               FROM tool_calls WHERE span_id = ?1"#,
        )?;

        let rows = stmt.query_map(params![span_id], |row| {
            let args_str: String = row.get(3)?;
            Ok(ToolCallRecord {
                call_id: row.get(0)?,
                span_id: row.get(1)?,
                tool_name: row.get(2)?,
                arguments: serde_json::from_str(&args_str).unwrap_or(serde_json::Value::Null),
                result: row.get(4)?,
                elapsed_ms: row.get(5)?,
            })
        })?;

        let mut calls = Vec::new();
        for row in rows {
            calls.push(row?);
        }

        Ok(calls)
    }

    /// Deletes a trace and all its spans and tool calls.
    pub fn delete_trace(&self, trace_id: &str) -> Result<(), StoreError> {
        let conn = self.conn.lock().map_err(|_| StoreError::Lock)?;

        // Delete tool calls for all spans in this trace
        conn.execute(
            r#"DELETE FROM tool_calls WHERE span_id IN
               (SELECT span_id FROM spans WHERE trace_id = ?1)"#,
            params![trace_id],
        )?;

        // Delete spans
        conn.execute("DELETE FROM spans WHERE trace_id = ?1", params![trace_id])?;

        // Delete trace
        conn.execute("DELETE FROM traces WHERE trace_id = ?1", params![trace_id])?;

        Ok(())
    }

    /// Gets aggregate metrics for the dashboard.
    pub fn get_metrics_summary(&self) -> Result<MetricsSummary, StoreError> {
        let conn = self.conn.lock().map_err(|_| StoreError::Lock)?;

        let mut stmt = conn.prepare(
            r#"SELECT
               COUNT(*) as total_traces,
               COALESCE(SUM(total_input_tokens), 0) as total_input_tokens,
               COALESCE(SUM(total_output_tokens), 0) as total_output_tokens,
               COALESCE(SUM(total_tool_calls), 0) as total_tool_calls,
               COALESCE(AVG(total_elapsed_ms), 0) as avg_latency_ms
               FROM traces"#,
        )?;

        let summary = stmt.query_row([], |row| {
            Ok(MetricsSummary {
                total_traces: row.get(0)?,
                total_input_tokens: row.get(1)?,
                total_output_tokens: row.get(2)?,
                total_tool_calls: row.get(3)?,
                avg_latency_ms: row.get(4)?,
            })
        })?;

        Ok(summary)
    }
}

/// Aggregate metrics summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSummary {
    pub total_traces: u64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_tool_calls: u64,
    pub avg_latency_ms: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_store_crud() {
        let store = TraceStore::in_memory().unwrap();

        let trace = TraceRecord {
            trace_id: "trace-1".to_string(),
            pipeline_id: "pipe-1".to_string(),
            pipeline_name: "Test Pipeline".to_string(),
            timestamp: 1700000000000,
            input: "Hello".to_string(),
            output: "World".to_string(),
            total_elapsed_ms: 500,
            total_input_tokens: 10,
            total_output_tokens: 20,
            total_tool_calls: 2,
            status: TraceStatus::Success,
        };

        store.insert_trace(&trace).unwrap();

        let retrieved = store.get_trace("trace-1").unwrap().unwrap();
        assert_eq!(retrieved.trace_id, "trace-1");
        assert_eq!(retrieved.pipeline_name, "Test Pipeline");

        let traces = store.list_traces(&TraceQuery::default()).unwrap();
        assert_eq!(traces.len(), 1);
    }

    #[test]
    fn test_spans_and_tool_calls() {
        let store = TraceStore::in_memory().unwrap();

        let trace = TraceRecord {
            trace_id: "trace-1".to_string(),
            pipeline_id: "pipe-1".to_string(),
            pipeline_name: "Test".to_string(),
            timestamp: 1700000000000,
            input: "Hi".to_string(),
            output: "Hello".to_string(),
            total_elapsed_ms: 100,
            total_input_tokens: 5,
            total_output_tokens: 10,
            total_tool_calls: 1,
            status: TraceStatus::Success,
        };
        store.insert_trace(&trace).unwrap();

        let span = SpanRecord {
            span_id: "span-1".to_string(),
            trace_id: "trace-1".to_string(),
            node_id: "node-1".to_string(),
            node_type: "llm".to_string(),
            start_time: 1700000000000,
            end_time: 1700000000100,
            input: "Hi".to_string(),
            output: "Hello".to_string(),
            input_tokens: 5,
            output_tokens: 10,
            tool_call_count: 1,
            iteration_count: 1,
        };
        store.insert_span(&span).unwrap();

        let tool_call = ToolCallRecord {
            call_id: "call-1".to_string(),
            span_id: "span-1".to_string(),
            tool_name: "search".to_string(),
            arguments: serde_json::json!({"query": "test"}),
            result: "result".to_string(),
            elapsed_ms: 50,
        };
        store.insert_tool_call(&tool_call).unwrap();

        let spans = store.get_spans("trace-1").unwrap();
        assert_eq!(spans.len(), 1);

        let calls = store.get_tool_calls("span-1").unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "search");
    }
}
