//! Demonstrates a router pipeline that routes to different handlers.
//!
//! Run with: cargo run --example router_pipeline

use fissio::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Build a pipeline with routing
    let config = PipelineConfig::builder("support", "Customer Support")
        .description("Routes customer queries to appropriate handlers")

        // Router classifies the input
        .node("router", NodeType::Router)
            .prompt("Classify the user's request as: technical, billing, or general")
            .done()

        // Technical support handler
        .node("technical", NodeType::Llm)
            .prompt("You are a technical support specialist. Help with technical issues.")
            .done()

        // Billing handler
        .node("billing", NodeType::Llm)
            .prompt("You are a billing specialist. Help with payment and account issues.")
            .done()

        // General inquiries handler
        .node("general", NodeType::Llm)
            .prompt("You are a customer service representative. Help with general questions.")
            .done()

        // Input goes to router
        .edge("input", "router")

        // Router conditionally routes to handlers
        .conditional_edge("router", &["technical", "billing", "general"])

        // All handlers output
        .edge("technical", "output")
        .edge("billing", "output")
        .edge("general", "output")
        .build();

    println!("Built pipeline: {}", config.name);
    println!("Nodes: {:?}", config.nodes.iter().map(|n| &n.id).collect::<Vec<_>>());

    // To actually run this, you would need to configure models:
    //
    // let model = ModelConfig { ... };
    // let engine = PipelineEngine::new(config, vec![model.clone()], model, HashMap::new());
    // let result = engine.execute_stream("My payment failed", &[]).await?;
    //
    // The router would classify "My payment failed" as "billing" and route there.

    println!("\nThis example builds the pipeline config.");
    println!("To execute, configure models and create a PipelineEngine.");

    Ok(())
}
