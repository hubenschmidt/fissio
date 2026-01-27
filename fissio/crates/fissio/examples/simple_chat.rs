//! Simple chat example using fissio.
//!
//! Run with: cargo run --example simple_chat

use fissio::prelude::*;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure the model
    let model = ModelConfig {
        id: "gpt-4".into(),
        name: "GPT-4".into(),
        model: "gpt-4-turbo".into(),
        api_base: None, // Uses OPENAI_API_KEY env var
    };

    // Build a simple pipeline with one LLM node
    let config = PipelineConfig::builder("chat", "Simple Chat")
        .node("assistant", NodeType::Llm)
            .prompt("You are a helpful assistant. Be concise.")
            .done()
        .edge("input", "assistant")
        .edge("assistant", "output")
        .build();

    // Create the engine
    let engine = PipelineEngine::new(
        config,
        vec![model.clone()],
        model,
        HashMap::new(),
    );

    // Execute the pipeline
    let result = engine.execute_stream("What is Rust?", &[]).await?;

    match result {
        EngineOutput::Complete(text) => println!("{}", text),
        EngineOutput::Stream(_stream) => {
            // For streaming, you would consume the stream
            println!("(Streaming not shown in this example)");
        }
    }

    Ok(())
}
