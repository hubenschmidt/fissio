//! Trace observability API handlers.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::Json;
use fissio_monitor::{SpanRecord, TraceQuery, TraceRecord, TraceStatus};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::ServerState;

/// Response for listing traces.
#[derive(Serialize)]
pub struct TracesListResponse {
    pub traces: Vec<TraceRecord>,
}

/// Response for a single trace with spans.
#[derive(Serialize)]
pub struct TraceDetailResponse {
    pub trace: TraceRecord,
    pub spans: Vec<SpanRecord>,
}

/// Query parameters for listing traces.
#[derive(Debug, Deserialize, Default)]
pub struct ListTracesQuery {
    pub pipeline_id: Option<String>,
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// GET /api/traces - List traces with optional filtering.
pub async fn list(
    State(state): State<Arc<ServerState>>,
    Query(params): Query<ListTracesQuery>,
) -> Result<Json<TracesListResponse>, AppError> {
    let query = TraceQuery {
        pipeline_id: params.pipeline_id,
        status: params.status.as_deref().map(TraceStatus::from_str),
        limit: params.limit.or(Some(50)),
        offset: params.offset,
    };

    let traces = state.trace_store.list_traces(&query).map_err(|e| {
        tracing::error!("Failed to list traces: {}", e);
        AppError::Internal("failed to list traces".into())
    })?;

    Ok(Json(TracesListResponse { traces }))
}

/// GET /api/traces/:id - Get a single trace with its spans.
pub async fn get(
    State(state): State<Arc<ServerState>>,
    Path(trace_id): Path<String>,
) -> Result<Json<TraceDetailResponse>, AppError> {
    let trace = state
        .trace_store
        .get_trace(&trace_id)
        .map_err(|e| {
            tracing::error!("Failed to get trace: {}", e);
            AppError::Internal("failed to get trace".into())
        })?
        .ok_or_else(|| AppError::NotFound("trace not found".into()))?;

    let spans = state.trace_store.get_spans(&trace_id).map_err(|e| {
        tracing::error!("Failed to get spans: {}", e);
        AppError::Internal("failed to get spans".into())
    })?;

    Ok(Json(TraceDetailResponse { trace, spans }))
}

/// DELETE /api/traces/:id - Delete a trace.
pub async fn delete(
    State(state): State<Arc<ServerState>>,
    Path(trace_id): Path<String>,
) -> Result<Json<()>, AppError> {
    state.trace_store.delete_trace(&trace_id).map_err(|e| {
        tracing::error!("Failed to delete trace: {}", e);
        AppError::Internal("failed to delete trace".into())
    })?;

    Ok(Json(()))
}

/// GET /api/metrics/summary - Get aggregate metrics.
pub async fn metrics_summary(
    State(state): State<Arc<ServerState>>,
) -> Result<Json<fissio_monitor::MetricsSummary>, AppError> {
    let summary = state.trace_store.get_metrics_summary().map_err(|e| {
        tracing::error!("Failed to get metrics summary: {}", e);
        AppError::Internal("failed to get metrics".into())
    })?;

    Ok(Json(summary))
}
