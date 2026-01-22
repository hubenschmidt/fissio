use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

// ─────────────────────────────────────────────────────────────────────────────
// Error
// ─────────────────────────────────────────────────────────────────────────────

#[derive(thiserror::Error, Debug)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to parse config: {0}")]
    Parse(#[from] serde_json::Error),

    #[error("Preset not found: {0}")]
    PresetNotFound(String),
}

// ─────────────────────────────────────────────────────────────────────────────
// Node Types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    Llm,
    Gate,
    Router,
    Coordinator,
    Aggregator,
    Orchestrator,
    Worker,
    Synthesizer,
    Evaluator,
}

// ─────────────────────────────────────────────────────────────────────────────
// Edge Types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    #[default]
    Direct,
    Dynamic,
    Conditional,
    Parallel,
}

// ─────────────────────────────────────────────────────────────────────────────
// Config Structs
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: NodeType,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub config: serde_json::Value,
    #[serde(default)]
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeConfig {
    pub from: EdgeEndpoint,
    pub to: EdgeEndpoint,
    #[serde(default)]
    pub edge_type: EdgeType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EdgeEndpoint {
    Single(String),
    Multiple(Vec<String>),
}

impl EdgeEndpoint {
    pub fn as_vec(&self) -> Vec<&str> {
        match self {
            EdgeEndpoint::Single(s) => vec![s.as_str()],
            EdgeEndpoint::Multiple(v) => v.iter().map(|s| s.as_str()).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub nodes: Vec<NodeConfig>,
    pub edges: Vec<EdgeConfig>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Preset Registry
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct PresetRegistry {
    presets: HashMap<String, PipelineConfig>,
}

impl PresetRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load_from_dir(dir: &Path) -> Result<Self, ConfigError> {
        let mut registry = Self::new();

        let entries = fs::read_dir(dir)?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                let content = fs::read_to_string(&path)?;
                let config: PipelineConfig = serde_json::from_str(&content)?;
                registry.presets.insert(config.id.clone(), config);
            }
        }

        Ok(registry)
    }

    pub fn get(&self, id: &str) -> Option<&PipelineConfig> {
        self.presets.get(id)
    }

    pub fn list(&self) -> Vec<&PipelineConfig> {
        self.presets.values().collect()
    }

    pub fn ids(&self) -> Vec<&str> {
        self.presets.keys().map(|s| s.as_str()).collect()
    }
}
