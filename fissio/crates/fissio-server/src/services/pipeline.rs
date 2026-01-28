//! Pipeline configuration persistence service.

use crate::dto::{PipelineInfo, SavePipelineRequest};
use crate::error::AppError;
use crate::ServerState;

/// Saves a pipeline to the database and updates the in-memory cache.
pub async fn save_pipeline(state: &ServerState, req: &SavePipelineRequest) -> Result<PipelineInfo, AppError> {
    // Persist to database
    {
        let db = state.db_lock()?;
        crate::db::save_pipeline(&db, req).map_err(|e| {
            AppError::Internal(format!("save failed: {}", e))
        })?;
    }

    // Build the new PipelineInfo
    let info = PipelineInfo {
        id: req.id.clone(),
        name: req.name.clone(),
        description: req.description.clone(),
        nodes: req.nodes.clone(),
        edges: req.edges.clone(),
        layout: req.layout.clone(),
    };

    // Update in-memory cache
    let mut configs = state.configs.write().await;
    if let Some(idx) = configs.iter().position(|p| p.id == info.id) {
        configs[idx] = info.clone();
    } else {
        configs.push(info.clone());
    }

    Ok(info)
}

/// Deletes a pipeline from the database and removes from in-memory cache.
pub async fn delete_pipeline(state: &ServerState, id: &str) -> Result<(), AppError> {
    // Delete from database
    {
        let db = state.db_lock()?;
        crate::db::delete_pipeline(&db, id).map_err(|e| {
            AppError::Internal(format!("delete failed: {}", e))
        })?;
    }

    // Remove from in-memory cache
    let mut configs = state.configs.write().await;
    configs.retain(|p| p.id != id);

    Ok(())
}
