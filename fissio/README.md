# Fissio

**Pipeline-first agent framework for LLM-powered systems.**

Fissio treats declarative pipeline definitions as the primary abstraction for building agent systems. Unlike agent-centric frameworks, fissio uses graph topology for orchestration with specialized node types.

## Installation

```toml
[dependencies]
fissio = "0.1"
```

## Quick Start

### Load from JSON

```rust
use fissio::prelude::*;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load pipeline from JSON file
    let config = PipelineConfig::from_file("pipeline.json")?;

    // Configure models
    let models = vec![ModelConfig {
        id: "gpt-4".into(),
        name: "GPT-4".into(),
        model: "gpt-4-turbo".into(),
        api_base: None,
    }];
    let default_model = models[0].clone();

    // Create engine and execute
    let engine = PipelineEngine::new(config, models, default_model, HashMap::new());
    let result = engine.execute_stream("Hello!", &[]).await?;

    match result {
        EngineOutput::Complete(text) => println!("{}", text),
        EngineOutput::Stream(_) => println!("Streaming response..."),
    }
    Ok(())
}
```

### Builder API

```rust
use fissio::prelude::*;

let config = PipelineConfig::builder("research", "Research Pipeline")
    .description("Searches the web and summarizes findings")
    .node("researcher", NodeType::Worker)
        .prompt("You are a research assistant. Search for information.")
        .tools(["web_search", "fetch_url"])
        .done()
    .node("summarizer", NodeType::Llm)
        .prompt("Summarize the research findings concisely.")
        .model("gpt-4")
        .done()
    .edge("input", "researcher")
    .edge("researcher", "summarizer")
    .edge("summarizer", "output")
    .build();
```

## Pipeline Definition (JSON)

```json
{
  "id": "research-pipeline",
  "name": "Research Assistant",
  "description": "Searches and summarizes information",
  "nodes": [
    {
      "id": "researcher",
      "type": "worker",
      "prompt": "You are a research assistant.",
      "tools": ["web_search", "fetch_url"]
    },
    {
      "id": "summarizer",
      "type": "llm",
      "prompt": "Summarize the findings concisely.",
      "model": "gpt-4"
    }
  ],
  "edges": [
    { "from": "input", "to": "researcher" },
    { "from": "researcher", "to": "summarizer" },
    { "from": "summarizer", "to": "output" }
  ]
}
```

## Node Types

| Type | Description | Tools |
|------|-------------|-------|
| `llm` | Simple LLM call with system prompt | No |
| `worker` | LLM with agentic tool loop | Yes |
| `router` | Classifies input, routes to targets | No |
| `gate` | Validates input before proceeding | No |
| `aggregator` | Combines outputs from multiple nodes | No |
| `orchestrator` | Dynamic task decomposition | No |
| `evaluator` | Quality scoring of outputs | No |
| `synthesizer` | Synthesizes multiple inputs | No |
| `coordinator` | Distributes work to workers | No |

## Edge Types

| Type | Description |
|------|-------------|
| `direct` | Sequential execution (default) |
| `parallel` | Concurrent execution of all targets |
| `conditional` | Router chooses which path to follow |
| `dynamic` | Orchestrator dynamically selects targets |

## Custom Tools

```rust
use fissio::{Tool, ToolError, ToolRegistry};
use async_trait::async_trait;

struct CalculatorTool;

#[async_trait]
impl Tool for CalculatorTool {
    fn name(&self) -> &str { "calculator" }

    fn description(&self) -> &str { "Performs math calculations" }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "Math expression to evaluate"
                }
            },
            "required": ["expression"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String, ToolError> {
        let expr = args["expression"].as_str()
            .ok_or_else(|| ToolError::InvalidArguments("missing expression".into()))?;
        // Evaluate expression...
        Ok("42".to_string())
    }
}

// Register custom tool
let mut registry = ToolRegistry::with_defaults();
registry.register(CalculatorTool);
```

## LLM Providers

Fissio supports multiple LLM providers through `UnifiedLlmClient`:

| Provider | Models | API Key Env Var |
|----------|--------|-----------------|
| OpenAI | `gpt-4`, `gpt-3.5-turbo`, etc. | `OPENAI_API_KEY` |
| Anthropic | `claude-3-*`, `claude-2`, etc. | `ANTHROPIC_API_KEY` |
| Ollama | Any local model | N/A (local) |

```rust
use fissio::UnifiedLlmClient;

// Auto-detects provider from model name
let client = UnifiedLlmClient::new("gpt-4", None);        // OpenAI
let client = UnifiedLlmClient::new("claude-3-opus", None); // Anthropic
let client = UnifiedLlmClient::new("llama2", Some("http://localhost:11434/v1")); // Ollama
```

## Crate Structure

| Crate | Description |
|-------|-------------|
| `fissio` | Facade crate (re-exports all) |
| `fissio-config` | Pipeline schema, builders, node/edge types |
| `fissio-core` | Error types, messages, model config |
| `fissio-engine` | DAG execution engine |
| `fissio-llm` | LLM provider clients |
| `fissio-tools` | Tool registry and built-in tools |

## Built-in Tools

- `fetch_url` — Fetches content from a URL
- `web_search` — Web search via Tavily API (requires `TAVILY_API_KEY`)

## License

MIT
