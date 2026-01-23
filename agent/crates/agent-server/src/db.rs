use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use tracing::{info, error};

use crate::dto::{EdgeInfo, NodeInfo, PipelineInfo, SavePipelineRequest};

pub fn init_db(path: &str) -> Result<Connection> {
    if let Some(parent) = Path::new(path).parent() {
        fs::create_dir_all(parent).context("failed to create db directory")?;
    }
    let conn = Connection::open(path).context("failed to open database")?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS user_pipelines (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            config_json TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );"
    ).context("failed to create table")?;
    info!("Database initialized at {}", path);
    Ok(conn)
}

pub fn list_user_pipelines(conn: &Connection) -> Vec<PipelineInfo> {
    let mut stmt = match conn.prepare("SELECT id, name, description, config_json FROM user_pipelines") {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to prepare list query: {}", e);
            return vec![];
        }
    };

    let rows = match stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let name: String = row.get(1)?;
        let description: String = row.get(2)?;
        let config_json: String = row.get(3)?;
        Ok((id, name, description, config_json))
    }) {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to query user pipelines: {}", e);
            return vec![];
        }
    };

    rows.filter_map(|row| {
        let (id, name, description, config_json) = row.ok()?;
        let config: StoredConfig = serde_json::from_str(&config_json).ok()?;
        Some(PipelineInfo {
            id,
            name,
            description,
            nodes: config.nodes,
            edges: config.edges,
        })
    }).collect()
}

pub fn save_pipeline(conn: &Connection, req: &SavePipelineRequest) -> Result<()> {
    let config = StoredConfig {
        nodes: req.nodes.clone(),
        edges: req.edges.clone(),
    };
    let config_json = serde_json::to_string(&config).context("failed to serialize config")?;
    conn.execute(
        "INSERT OR REPLACE INTO user_pipelines (id, name, description, config_json, updated_at)
         VALUES (?1, ?2, ?3, ?4, datetime('now'))",
        params![req.id, req.name, req.description, config_json],
    ).context("failed to save pipeline")?;
    info!("Saved pipeline config: {} ({})", req.name, req.id);
    Ok(())
}

pub fn delete_pipeline(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM user_pipelines WHERE id = ?1", params![id])
        .context("failed to delete pipeline")?;
    info!("Deleted pipeline config: {}", id);
    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize)]
struct StoredConfig {
    nodes: Vec<NodeInfo>,
    edges: Vec<EdgeInfo>,
}
