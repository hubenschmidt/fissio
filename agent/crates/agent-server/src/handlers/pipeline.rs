use std::sync::Arc;

use axum::{extract::State, Json};
use tracing::{info, error};

use crate::dto::{DeletePipelineRequest, PipelineInfo, SavePipelineRequest, SavePipelineResponse};
use crate::error::AppError;
use crate::ServerState;

pub async fn list(
    State(state): State<Arc<ServerState>>,
) -> Json<Vec<PipelineInfo>> {
    let configs = state.configs.read().await;
    Json(configs.clone())
}

pub async fn save(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<SavePipelineRequest>,
) -> Result<Json<SavePipelineResponse>, AppError> {
    info!("Saving pipeline config: {} ({})", req.name, req.id);

    let db_result = {
        let db = state.db.lock().map_err(|e| {
            error!("DB lock poisoned: {}", e);
            AppError::Internal("database lock error".into())
        })?;
        crate::db::save_pipeline(&db, &req)
    };

    if let Err(e) = db_result {
        error!("Failed to save pipeline: {}", e);
        return Err(AppError::Internal(format!("save failed: {}", e)));
    }

    let new_info = PipelineInfo {
        id: req.id.clone(),
        name: req.name,
        description: req.description,
        nodes: req.nodes,
        edges: req.edges,
    };

    let mut configs = state.configs.write().await;
    let existing = configs.iter().position(|p| p.id == new_info.id);
    match existing {
        Some(idx) => configs[idx] = new_info,
        None => configs.push(new_info),
    }

    info!("Pipeline config saved successfully: {}", req.id);
    Ok(Json(SavePipelineResponse { success: true, id: req.id }))
}

pub async fn delete(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<DeletePipelineRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    info!("Deleting pipeline config: {}", req.id);

    let db_result = {
        let db = state.db.lock().map_err(|e| {
            error!("DB lock poisoned: {}", e);
            AppError::Internal("database lock error".into())
        })?;
        crate::db::delete_pipeline(&db, &req.id)
    };

    if let Err(e) = db_result {
        error!("Failed to delete pipeline: {}", e);
        return Err(AppError::Internal(format!("delete failed: {}", e)));
    }

    let mut configs = state.configs.write().await;
    configs.retain(|p| p.id != req.id);

    Ok(Json(serde_json::json!({ "success": true })))
}
