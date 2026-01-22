use std::sync::Arc;
use std::time::Instant;

use agent_core::{Message as CoreMessage, ModelConfig};
use agent_llm::{LlmStream, OllamaClient, OllamaMetrics, StreamChunk};
use agent_pipeline::{PipelineRunner, StreamResponse};
use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    response::IntoResponse,
};
use futures::{stream::SplitSink, SinkExt, StreamExt};
use serde::Serialize;
use tracing::{error, info};

use crate::dto::{InitResponse, WsMetadata, WsPayload, WsResponse};
use crate::services::model;
use crate::ServerState;

struct StreamResult {
    input_tokens: u32,
    output_tokens: u32,
    ollama_metrics: Option<OllamaMetrics>,
}

async fn send_json<T: Serialize>(sender: &mut SplitSink<WebSocket, Message>, data: &T) -> bool {
    let Ok(json) = serde_json::to_string(data) else {
        error!("JSON serialization failed");
        return false;
    };
    sender.send(Message::Text(json.into())).await.is_ok()
}

async fn consume_stream(
    sender: &mut SplitSink<WebSocket, Message>,
    mut stream: LlmStream,
) -> (String, u32, u32) {
    let mut accumulated = String::new();
    let mut input_tokens = 0u32;
    let mut output_tokens = 0u32;

    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(StreamChunk::Content(chunk)) => {
                accumulated.push_str(&chunk);
                if !send_json(sender, &WsResponse::stream(&chunk)).await {
                    break;
                }
            }
            Ok(StreamChunk::Usage { input_tokens: i, output_tokens: o }) => {
                input_tokens = i;
                output_tokens = o;
            }
            Err(e) => {
                error!("Stream error: {}", e);
                break;
            }
        }
    }
    (accumulated, input_tokens, output_tokens)
}

async fn send_error(sender: &mut SplitSink<WebSocket, Message>) -> String {
    let error_msg = "Sorry—there was an error generating the response.";
    let _ = send_json(sender, &WsResponse::stream(error_msg)).await;
    error_msg.to_string()
}

async fn process_ollama(
    sender: &mut SplitSink<WebSocket, Message>,
    model: &ModelConfig,
    history: &[CoreMessage],
    message: &str,
) -> StreamResult {
    let api_base = model.api_base.as_ref().expect("ollama requires api_base");
    let client = OllamaClient::new(&model.model, api_base);
    info!("Using native Ollama API for verbose metrics");

    let result = client
        .chat_stream_with_metrics("You are a helpful assistant.", history, message)
        .await;

    match result {
        Ok((stream, metrics_collector)) => {
            let (_content, input_tokens, output_tokens) = consume_stream(sender, Box::pin(stream)).await;
            StreamResult {
                input_tokens,
                output_tokens,
                ollama_metrics: Some(metrics_collector.get_metrics()),
            }
        }
        Err(e) => {
            error!("Ollama error: {}", e);
            send_error(sender).await;
            StreamResult {
                input_tokens: 0,
                output_tokens: 0,
                ollama_metrics: None,
            }
        }
    }
}

async fn process_pipeline(
    sender: &mut SplitSink<WebSocket, Message>,
    pipeline: &PipelineRunner,
    message: &str,
    history: &[CoreMessage],
    model: &ModelConfig,
) -> StreamResult {
    let stream_result = pipeline.process_stream(message, history, model).await;

    match stream_result {
        Ok(StreamResponse::Stream(stream)) => {
            let (_content, input_tokens, output_tokens) = consume_stream(sender, stream).await;
            StreamResult { input_tokens, output_tokens, ollama_metrics: None }
        }
        Ok(StreamResponse::Complete(response)) => {
            let _ = send_json(sender, &WsResponse::stream(&response)).await;
            StreamResult { input_tokens: 0, output_tokens: 0, ollama_metrics: None }
        }
        Err(e) => {
            error!("Pipeline error: {}", e);
            send_error(sender).await;
            StreamResult {
                input_tokens: 0,
                output_tokens: 0,
                ollama_metrics: None,
            }
        }
    }
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<ServerState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<ServerState>) {
    let (mut sender, mut receiver) = socket.split();

    // Wait for init message first
    let uuid = loop {
        let Some(Ok(msg)) = receiver.next().await else {
            return;
        };
        let Message::Text(text) = msg else { continue };

        let payload: WsPayload = match serde_json::from_str(&text) {
            Ok(p) => p,
            Err(e) => {
                error!("JSON parse error: {}", e);
                continue;
            }
        };

        if !payload.init {
            continue;
        }

        let uuid = payload.uuid.unwrap_or_else(|| "anonymous".to_string());
        info!("Connection initialized: {}", uuid);

        let init_resp = InitResponse { models: state.models.clone() };
        if !send_json(&mut sender, &init_resp).await {
            return;
        }
        break uuid;
    };

    // Process messages with immutable uuid
    while let Some(Ok(msg)) = receiver.next().await {
        let Message::Text(text) = msg else { continue };

        let payload: WsPayload = match serde_json::from_str(&text) {
            Ok(p) => p,
            Err(e) => {
                error!("JSON parse error: {}", e);
                continue;
            }
        };

        if let Some(wake_model_id) = &payload.wake_model_id {
            if !send_json(&mut sender, &WsResponse::model_status("loading")).await {
                break;
            }
            let prev = payload.unload_model_id.as_deref();
            match model::warmup(&state, wake_model_id, prev).await {
                Ok(m) => info!("Model {} ready via WebSocket", m.name),
                Err(e) => error!("Wake failed: {:?}", e),
            }
            if !send_json(&mut sender, &WsResponse::model_status("ready")).await {
                break;
            }
            continue;
        }

        if let Some(unload_model_id) = &payload.unload_model_id {
            if !send_json(&mut sender, &WsResponse::model_status("unloading")).await {
                break;
            }
            if let Err(e) = model::unload(&state, unload_model_id).await {
                error!("Unload failed: {:?}", e);
            }
            if !send_json(&mut sender, &WsResponse::model_status("ready")).await {
                break;
            }
            continue;
        }

        let Some(message) = payload.message else {
            continue;
        };

        let model_id = payload.model_id.as_deref().unwrap_or("");
        let model = state.get_model(model_id);

        info!(
            "Message from {} (model: {}): {}...",
            uuid,
            model.name,
            message.get(..50).unwrap_or(&message)
        );

        let history = payload.history;

        let start = Instant::now();
        let use_ollama_native = payload.verbose && model.api_base.is_some();

        let result = match use_ollama_native {
            true => process_ollama(&mut sender, &model, &history, &message).await,
            false => process_pipeline(&mut sender, &state.pipeline, &message, &history, &model).await,
        };

        let elapsed_ms = start.elapsed().as_millis() as u64;

        let metadata = match result.ollama_metrics {
            Some(m) => {
                info!(
                    "Ollama metrics: {:.1} tok/s, {} tokens, {}ms total",
                    m.tokens_per_sec(),
                    m.eval_count,
                    m.total_duration_ms()
                );
                WsMetadata {
                    input_tokens: m.prompt_eval_count,
                    output_tokens: m.eval_count,
                    elapsed_ms,
                    load_duration_ms: Some(m.load_duration_ms()),
                    prompt_eval_ms: Some(m.prompt_eval_ms()),
                    eval_ms: Some(m.eval_ms()),
                    tokens_per_sec: Some(m.tokens_per_sec()),
                }
            }
            None => WsMetadata {
                input_tokens: result.input_tokens,
                output_tokens: result.output_tokens,
                elapsed_ms,
                ..Default::default()
            },
        };

        info!("Sending metadata: {:?}", metadata);
        if !send_json(&mut sender, &WsResponse::end_with_metadata(metadata)).await {
            break;
        }
    }

    info!("Connection closed: {}", uuid);
}
