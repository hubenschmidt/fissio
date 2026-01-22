use std::env;
use std::sync::Arc;

use agents_core::{Message, MessageRole, ModelConfig};
use agents_llm::discover_models;
use agents_pipeline::{Evaluator, Frontline, Orchestrator, PipelineRunner};
use agents_workers::{EmailWorker, GeneralWorker, SearchWorker, WorkerRegistry};
use dashmap::DashMap;
use tracing::{info, warn};

const OLLAMA_HOST: &str = "http://host.docker.internal:11434";

fn cloud_models() -> Vec<ModelConfig> {
    vec![ModelConfig {
        id: "openai-gpt4o".into(),
        name: "GPT-4o (OpenAI)".into(),
        model: "gpt-4o".into(),
        api_base: None,
    }]
}

pub struct AppState {
    pub pipeline: PipelineRunner,
    pub conversations: DashMap<String, Vec<Message>>,
    pub models: Vec<ModelConfig>,
}

impl AppState {
    pub async fn new() -> Self {
        // Start model discovery immediately (network I/O)
        let discovery_future = discover_models(OLLAMA_HOST);

        // Do all sync initialization while discovery runs
        let serper_key = env::var("SERPER_API_KEY").unwrap_or_default();
        let sendgrid_key = env::var("SENDGRID_API_KEY").unwrap_or_default();
        let from_email = env::var("SENDGRID_FROM_EMAIL").unwrap_or_else(|_| "noreply@example.com".into());

        let frontline = Frontline::new();
        let orchestrator = Orchestrator::new();
        let evaluator = Evaluator::new();

        let general_worker = GeneralWorker::new();
        let search_worker = SearchWorker::new(serper_key.clone()).ok();
        let email_worker = EmailWorker::new(sendgrid_key.clone(), from_email.clone()).ok();

        let mut workers = WorkerRegistry::new();
        workers.register(Arc::new(GeneralWorker::new()));

        match SearchWorker::new(serper_key) {
            Ok(w) => workers.register(Arc::new(w)),
            Err(_) => warn!("SearchWorker disabled: SERPER_API_KEY not configured"),
        }

        match EmailWorker::new(sendgrid_key, from_email) {
            Ok(w) => workers.register(Arc::new(w)),
            Err(_) => warn!("EmailWorker disabled: SENDGRID_API_KEY not configured"),
        }

        let pipeline = PipelineRunner::new(
            frontline,
            orchestrator,
            evaluator,
            workers,
            general_worker,
            search_worker,
            email_worker,
        );

        // Now await discovery results
        let mut models = cloud_models();
        match discovery_future.await {
            Ok(ollama_models) => {
                info!("Found {} local Ollama models", ollama_models.len());
                for m in &ollama_models {
                    info!("  - {} ({})", m.name, m.id);
                }
                models.extend(ollama_models);
            }
            Err(e) => {
                warn!("Ollama discovery failed (is Ollama running?): {}", e);
            }
        }

        Self {
            pipeline,
            conversations: DashMap::new(),
            models,
        }
    }

    pub fn get_model(&self, model_id: &str) -> ModelConfig {
        self.models
            .iter()
            .find(|m| m.id == model_id)
            .or_else(|| self.models.first())
            .cloned()
            .expect("at least one model must be configured")
    }

    pub fn get_conversation(&self, uuid: &str) -> Vec<Message> {
        self.conversations
            .get(uuid)
            .map(|v| v.clone())
            .unwrap_or_default()
    }

    pub fn add_message(&self, uuid: &str, role: MessageRole, content: &str) {
        self.conversations
            .entry(uuid.to_string())
            .or_default()
            .push(Message {
                role,
                content: content.to_string(),
            });
    }
}

