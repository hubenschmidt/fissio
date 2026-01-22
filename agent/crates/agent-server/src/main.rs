mod dto;
mod error;
mod handlers;
mod services;
mod ws;

use std::env;
use std::sync::Arc;
use std::time::Duration;

use agent_core::ModelConfig;
use agent_llm::discover_models;
use agent_pipeline::{
    EmailWorker, Evaluator, Frontline, GeneralWorker, Orchestrator, PipelineRunner, SearchWorker,
    WorkerRegistry,
};
use anyhow::Result;
use axum::body::Body;
use axum::http::{Request, Response};
use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
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

pub struct ServerState {
    pub pipeline: PipelineRunner,
    pub models: Vec<ModelConfig>,
}

impl ServerState {
    pub fn get_model(&self, model_id: &str) -> ModelConfig {
        self.models
            .iter()
            .find(|m| m.id == model_id)
            .or_else(|| self.models.first())
            .cloned()
            .expect("at least one model must be configured")
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_target(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".parse().unwrap()),
        )
        .compact()
        .init();

    let state = Arc::new(init_server_state().await);

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(|req: &Request<Body>| {
            tracing::info_span!(
                "request",
                method = %req.method(),
                uri = %req.uri(),
                version = ?req.version(),
            )
        })
        .on_response(|res: &Response<Body>, latency: Duration, _span: &tracing::Span| {
            info!(
                latency = %format!("{} ms", latency.as_millis()),
                status = %res.status().as_u16(),
                "finished processing request"
            );
        });

    let logged_routes = Router::new()
        .route("/ws", get(ws::ws_handler))
        .route("/wake", post(handlers::model::wake))
        .route("/unload", post(handlers::model::unload))
        .layer(trace_layer);

    let app = Router::new()
        .merge(logged_routes)
        .route("/health", get(handlers::health))
        .layer(cors)
        .with_state(state);

    let addr = "0.0.0.0:8000";
    info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn init_server_state() -> ServerState {
    let discovery_future = discover_models(OLLAMA_HOST);

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

    ServerState { pipeline, models }
}
