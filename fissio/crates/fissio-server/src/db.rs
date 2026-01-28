//! SQLite persistence layer for user-saved pipeline configurations.
//!
//! Provides CRUD operations for pipeline configs and seeds example data on first run.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use tracing::{error, info};

use std::collections::HashMap;
use crate::dto::{EdgeInfo, NodeInfo, PipelineInfo, Position, SavePipelineRequest};

/// Initializes the database, creating tables if needed.
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

/// Lists all user-saved pipeline configurations.
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
            layout: config.layout,
        })
    }).collect()
}

/// Saves or updates a pipeline configuration.
pub fn save_pipeline(conn: &Connection, req: &SavePipelineRequest) -> Result<()> {
    let config = StoredConfig {
        nodes: req.nodes.clone(),
        edges: req.edges.clone(),
        layout: req.layout.clone(),
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

/// Deletes a pipeline configuration by ID.
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    layout: Option<HashMap<String, Position>>,
}

/// Example pipeline definition loaded from JSON.
#[derive(serde::Deserialize)]
struct ExamplePipeline {
    id: String,
    name: String,
    description: String,
    nodes: Vec<NodeInfo>,
    edges: Vec<EdgeInfo>,
}

/// Seed example configs if the database is empty.
/// Loads examples from examples.json file.
pub fn seed_examples(conn: &Connection) -> Result<()> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM user_pipelines", [], |r| r.get(0))?;
    if count > 0 {
        info!("Database already has {} configs, skipping seed", count);
        return Ok(());
    }

    info!("Seeding example configs...");

    let examples_path = std::env::var("EXAMPLES_JSON")
        .unwrap_or_else(|_| "examples.json".to_string());
    let json_content = fs::read_to_string(&examples_path)
        .with_context(|| format!("failed to read {}", examples_path))?;
    let examples: Vec<ExamplePipeline> = serde_json::from_str(&json_content)
        .context("failed to parse examples.json")?;

    let example_count = examples.len();
    for ex in examples {
        let config = StoredConfig { nodes: ex.nodes, edges: ex.edges, layout: None };
        let config_json = serde_json::to_string(&config)?;

        conn.execute(
            "INSERT INTO user_pipelines (id, name, description, config_json) VALUES (?1, ?2, ?3, ?4)",
            params![ex.id, ex.name, ex.description, config_json],
        )?;
        info!("  Seeded: {}", ex.name);
    }

    info!("Seeded {} example configs", example_count);
    Ok(())
}
