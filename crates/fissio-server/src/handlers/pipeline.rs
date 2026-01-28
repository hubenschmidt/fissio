//! Pipeline CRUD HTTP handlers.

use std::sync::Arc;

use axum::{extract::State, Json};
use tracing::{error, info};

use crate::dto::{DeletePipelineRequest, PipelineInfo, SavePipelineRequest, SavePipelineResponse};
use crate::error::AppError;
use crate::services::pipeline as pipeline_service;
use crate::ServerState;

/// Lists all saved pipeline configurations.
pub async fn list(
    State(state): State<Arc<ServerState>>,
) -> Json<Vec<PipelineInfo>> {
    let configs = state.configs.read().await;
    Json(configs.clone())
}

/// Saves a pipeline configuration.
pub async fn save(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<SavePipelineRequest>,
) -> Result<Json<SavePipelineResponse>, AppError> {
    info!("Saving pipeline config: {} ({})", req.name, req.id);

    pipeline_service::save_pipeline(&state, &req).await.map_err(|e| {
        error!("Failed to save pipeline: {:?}", e);
        e
    })?;

    info!("Pipeline config saved successfully: {}", req.id);
    Ok(Json(SavePipelineResponse { success: true, id: req.id }))
}

/// Deletes a pipeline configuration.
pub async fn delete(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<DeletePipelineRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    info!("Deleting pipeline config: {}", req.id);

    pipeline_service::delete_pipeline(&state, &req.id).await.map_err(|e| {
        error!("Failed to delete pipeline: {:?}", e);
        e
    })?;

    Ok(Json(serde_json::json!({ "success": true })))
}
