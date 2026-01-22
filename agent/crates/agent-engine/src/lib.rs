use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use agent_config::{EdgeConfig, EdgeEndpoint, EdgeType, NodeConfig, NodeType, PipelineConfig};
use agent_core::{AgentError, ModelConfig};
use agent_network::{LlmClient, LlmStream};
use futures::future::join_all;
use tokio::sync::RwLock;
use async_recursion::async_recursion;
use tracing::{info, debug};

// ─────────────────────────────────────────────────────────────────────────────
// Engine Types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct NodeInput {
    pub user_input: String,
    pub history: Vec<agent_core::Message>,
    pub context: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct NodeOutput {
    pub content: String,
    pub next_nodes: Vec<String>,
}

pub enum EngineOutput {
    Stream(LlmStream),
    Complete(String),
}

// ─────────────────────────────────────────────────────────────────────────────
// Model Resolver
// ─────────────────────────────────────────────────────────────────────────────

pub struct ModelResolver {
    models: HashMap<String, ModelConfig>,
    default_model: ModelConfig,
}

impl ModelResolver {
    pub fn new(models: Vec<ModelConfig>, default: ModelConfig) -> Self {
        let map = models.into_iter().map(|m| (m.id.clone(), m)).collect();
        Self { models: map, default_model: default }
    }

    pub fn resolve(&self, model_id: Option<&str>) -> &ModelConfig {
        model_id
            .and_then(|id| self.models.get(id))
            .unwrap_or(&self.default_model)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Pipeline Engine
// ─────────────────────────────────────────────────────────────────────────────

pub struct PipelineEngine {
    config: PipelineConfig,
    resolver: ModelResolver,
    node_overrides: HashMap<String, String>,
}

impl PipelineEngine {
    pub fn new(
        config: PipelineConfig,
        models: Vec<ModelConfig>,
        default_model: ModelConfig,
        node_overrides: HashMap<String, String>,
    ) -> Self {
        Self {
            config,
            resolver: ModelResolver::new(models, default_model),
            node_overrides,
        }
    }

    fn get_node_model(&self, node: &NodeConfig) -> &ModelConfig {
        let model_id = self.node_overrides
            .get(&node.id)
            .or(node.model.as_ref());
        self.resolver.resolve(model_id.map(|s| s.as_str()))
    }

    fn get_node(&self, id: &str) -> Option<&NodeConfig> {
        self.config.nodes.iter().find(|n| n.id == id)
    }

    fn get_outgoing_edges(&self, node_id: &str) -> Vec<&EdgeConfig> {
        self.config.edges.iter().filter(|e| {
            e.from.as_vec().contains(&node_id)
        }).collect()
    }

    pub async fn execute_stream(
        &self,
        user_input: &str,
        history: &[agent_core::Message],
    ) -> Result<EngineOutput, AgentError> {
        info!("╔══════════════════════════════════════════════════════════════");
        info!("║ PIPELINE: {}", self.config.name);
        info!("║ Input: {}...", user_input.chars().take(50).collect::<String>());
        info!("╠══════════════════════════════════════════════════════════════");

        if !self.node_overrides.is_empty() {
            info!("║ Node model overrides: {:?}", self.node_overrides);
        }

        let context = Arc::new(RwLock::new(HashMap::<String, String>::new()));
        context.write().await.insert("input".to_string(), user_input.to_string());

        let mut executed: HashSet<String> = HashSet::new();
        let step = Arc::new(RwLock::new(0usize));

        // Find starting edges (from "input")
        let start_edges: Vec<&EdgeConfig> = self.config.edges.iter()
            .filter(|e| matches!(&e.from, EdgeEndpoint::Single(s) if s == "input"))
            .collect();

        // Process graph
        for start_edge in start_edges {
            self.process_edge(start_edge, &context, &mut executed, history, &step).await?;
        }

        // Find output and return
        let ctx = context.read().await;

        // Find what feeds into output
        for edge in &self.config.edges {
            if matches!(&edge.to, EdgeEndpoint::Single(s) if s == "output") {
                let from_nodes = edge.from.as_vec();
                let output = from_nodes.iter()
                    .filter_map(|id| ctx.get(*id))
                    .last()
                    .cloned()
                    .unwrap_or_default();

                info!("║ Pipeline complete");
                info!("╚══════════════════════════════════════════════════════════════");
                return Ok(EngineOutput::Complete(output));
            }
        }

        info!("║ Pipeline complete (no output edge found)");
        info!("╚══════════════════════════════════════════════════════════════");
        Ok(EngineOutput::Complete(String::new()))
    }

    #[async_recursion]
    async fn process_edge(
        &self,
        edge: &EdgeConfig,
        context: &Arc<RwLock<HashMap<String, String>>>,
        executed: &mut HashSet<String>,
        history: &[agent_core::Message],
        step: &Arc<RwLock<usize>>,
    ) -> Result<(), AgentError> {
        let target_ids = edge.to.as_vec();

        // Skip if going to output
        if target_ids.len() == 1 && target_ids[0] == "output" {
            return Ok(());
        }

        match edge.edge_type {
            EdgeType::Parallel => {
                info!("╠══════════════════════════════════════════════════════════════");
                info!("║ PARALLEL EXECUTION: {:?}", target_ids);

                // Gather node data and inputs first (sequentially)
                let mut node_data = Vec::new();
                for id in target_ids.iter().filter(|&id| !executed.contains(*id)) {
                    let Some(node) = self.get_node(id) else { continue };
                    let input = self.get_input_for_node_async(id, context).await;
                    let model = self.get_node_model(node).clone();
                    node_data.push((node.id.clone(), node.node_type, model, node.prompt.clone(), input));
                }

                // Execute all targets in parallel
                let futures: Vec<_> = node_data.into_iter()
                    .map(|(node_id, node_type, model, prompt, input)| {
                        let step = Arc::clone(step);

                        async move {
                            let mut s = step.write().await;
                            *s += 1;
                            let current_step = *s;
                            drop(s);

                            let result = Self::execute_node_static(
                                &node_id, node_type, &model, prompt.as_deref(), &input, current_step
                            ).await;

                            (node_id, result)
                        }
                    })
                    .collect();

                let results = join_all(futures).await;

                // Store results
                for (node_id, result) in results {
                    match result {
                        Ok(output) => {
                            context.write().await.insert(node_id.clone(), output.content);
                            executed.insert(node_id);
                        }
                        Err(e) => return Err(e),
                    }
                }

                info!("║ PARALLEL EXECUTION COMPLETE");
                info!("╠══════════════════════════════════════════════════════════════");

                // Find edges that come FROM the parallel nodes and go to aggregator
                for node_id in target_ids {
                    for next_edge in self.get_outgoing_edges(node_id) {
                        // Only process once (check if any target already executed)
                        let next_targets = next_edge.to.as_vec();
                        let any_executed = next_targets.iter().any(|t| executed.contains(*t));
                        if !any_executed {
                            self.process_edge(next_edge, context, executed, history, step).await?;
                        }
                    }
                }
            }
            _ => {
                // Sequential execution
                for node_id in target_ids {
                    if executed.contains(node_id) || node_id == "output" {
                        continue;
                    }

                    let Some(node) = self.get_node(node_id) else { continue };

                    let input = self.get_input_for_node_async(node_id, context).await;

                    let mut s = step.write().await;
                    *s += 1;
                    let current_step = *s;
                    drop(s);

                    let model = self.get_node_model(node);
                    let output = Self::execute_node_static(
                        node_id, node.node_type, model, node.prompt.as_deref(), &input, current_step
                    ).await?;

                    context.write().await.insert(node_id.to_string(), output.content);
                    executed.insert(node_id.to_string());

                    // Process outgoing edges
                    for next_edge in self.get_outgoing_edges(node_id) {
                        self.process_edge(next_edge, context, executed, history, step).await?;
                    }
                }
            }
        }

        Ok(())
    }

    async fn get_input_for_node_async(&self, node_id: &str, context: &Arc<RwLock<HashMap<String, String>>>) -> String {
        let ctx = context.read().await;

        for edge in &self.config.edges {
            let to_nodes = edge.to.as_vec();
            if to_nodes.contains(&node_id) {
                let from_nodes = edge.from.as_vec();
                let inputs: Vec<String> = from_nodes
                    .iter()
                    .filter_map(|id| ctx.get(*id).cloned())
                    .collect();
                if !inputs.is_empty() {
                    return inputs.join("\n\n---\n\n");
                }
            }
        }
        ctx.get("input").cloned().unwrap_or_default()
    }

    async fn execute_node_static(
        node_id: &str,
        node_type: NodeType,
        model: &ModelConfig,
        prompt: Option<&str>,
        input: &str,
        step: usize,
    ) -> Result<NodeOutput, AgentError> {
        let client = LlmClient::new(&model.model, model.api_base.as_deref());

        info!("╠──────────────────────────────────────────────────────────────");
        info!("║ [{}] NODE: {} ({:?})", step, node_id, node_type);
        info!("║     Model: {}", model.name);
        debug!("║     Input: {}...", input.chars().take(100).collect::<String>());

        let start = std::time::Instant::now();

        let result = match node_type {
            NodeType::Llm => {
                let p = prompt.unwrap_or("");
                info!("║     → Calling LLM...");
                let response = client.chat(p, input).await?;
                info!("║     ← Response: {} chars", response.content.len());
                Ok(NodeOutput { content: response.content, next_nodes: vec![] })
            }
            NodeType::Gate => {
                info!("║     → Gate check (POC: always pass)");
                Ok(NodeOutput { content: input.to_string(), next_nodes: vec![] })
            }
            NodeType::Router => {
                info!("║     → Routing (POC: passthrough)");
                Ok(NodeOutput { content: input.to_string(), next_nodes: vec![] })
            }
            NodeType::Coordinator => {
                info!("║     → Coordinating (POC: passthrough)");
                Ok(NodeOutput { content: input.to_string(), next_nodes: vec![] })
            }
            NodeType::Orchestrator => {
                info!("║     → Orchestrating (POC: passthrough)");
                Ok(NodeOutput { content: input.to_string(), next_nodes: vec![] })
            }
            NodeType::Aggregator => {
                info!("║     → Aggregating inputs");
                Ok(NodeOutput { content: input.to_string(), next_nodes: vec![] })
            }
            NodeType::Synthesizer => {
                info!("║     → Synthesizing (POC: passthrough)");
                Ok(NodeOutput { content: input.to_string(), next_nodes: vec![] })
            }
            NodeType::Worker => {
                let p = prompt.unwrap_or("");
                info!("║     → Worker executing...");
                let response = client.chat(p, input).await?;
                info!("║     ← Response: {} chars", response.content.len());
                Ok(NodeOutput { content: response.content, next_nodes: vec![] })
            }
            NodeType::Evaluator => {
                info!("║     → Evaluating (POC: always pass)");
                Ok(NodeOutput { content: input.to_string(), next_nodes: vec![] })
            }
        };

        let elapsed = start.elapsed();
        info!("║     ✓ Completed in {:?}", elapsed);

        result
    }
}
