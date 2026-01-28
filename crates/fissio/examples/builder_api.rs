//! Demonstrates the PipelineConfig builder API.
//!
//! Run with: cargo run --example builder_api

use fissio::prelude::*;

fn main() {
    // Build a research pipeline using the fluent API
    let config = PipelineConfig::builder("research", "Research Pipeline")
        .description("Searches the web and summarizes findings")

        // Worker node with tools
        .node("researcher", NodeType::Worker)
            .prompt("You are a research assistant. Use the tools to find information.")
            .tools(["web_search", "fetch_url"])
            .done()

        // Simple LLM node for summarization
        .node("summarizer", NodeType::Llm)
            .prompt("Summarize the research findings in 2-3 sentences.")
            .model("gpt-4")
            .done()

        // Define the data flow
        .edge("input", "researcher")
        .edge("researcher", "summarizer")
        .edge("summarizer", "output")
        .build();

    // Print the configuration
    println!("Pipeline: {} ({})", config.name, config.id);
    println!("Description: {}", config.description);
    println!("\nNodes:");
    for node in &config.nodes {
        println!("  - {} ({:?})", node.id, node.node_type);
        if let Some(prompt) = &node.prompt {
            println!("    Prompt: {}...", &prompt[..prompt.len().min(50)]);
        }
        if !node.tools.is_empty() {
            println!("    Tools: {:?}", node.tools);
        }
    }
    println!("\nEdges:");
    for edge in &config.edges {
        println!("  {:?} -> {:?} ({:?})", edge.from, edge.to, edge.edge_type);
    }

    // Serialize to JSON
    let json = config.to_json().expect("serialization failed");
    println!("\nJSON:\n{}", json);
}
