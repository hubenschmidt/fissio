# svelte-rust-agents-sdk

A multi-agent chat system with a Svelte 5 frontend and Rust backend.

## Architecture

```
┌─────────────────┐     WebSocket      ┌─────────────────────────────────────┐
│                 │◄──────────────────►│              Backend                │
│  Svelte 5 UI    │                    │  ┌─────────────────────────────┐    │
│  (SvelteKit)    │                    │  │        Orchestrator         │    │
│                 │                    │  └──────────────┬──────────────┘    │
└─────────────────┘                    │                 │                   │
                                       │  ┌──────────────▼──────────────┐    │
                                       │  │          Workers            │    │
                                       │  │  • Search (SerpAPI)         │    │
                                       │  │  • Email (SendGrid)         │    │
                                       │  │  • General                  │    │
                                       │  └─────────────────────────────┘    │
                                       └─────────────────────────────────────┘
```

**Backend Crates:**
- `agents-core` — shared types and error handling
- `agents-llm` — OpenAI client wrapper
- `agents-workers` — worker implementations and registry
- `agents-server` — axum WebSocket server

## Prerequisites

- Docker & Docker Compose
- Node.js 22+ (for local frontend dev)
- Rust 1.75+ (for local backend dev)

## Environment Variables

Create a `.env` file in the project root:

```env
# Required
OPENAI_API_KEY=sk-...

# Optional
OPENAI_MODEL=gpt-4o
WORKER_MODEL=gpt-4o-mini
SERPAPI_KEY=...
SENDGRID_API_KEY=...
SENDGRID_FROM_EMAIL=noreply@example.com
RUST_LOG=info
```

## Running

### Development (with hot-reload)

```bash
docker compose up --build
```

Frontend available at `http://localhost:3000`, backend at `http://localhost:8000`.

### Production

```bash
docker compose -f docker-compose.yml up --build
```

## Project Structure

```
├── backend/
│   ├── crates/
│   │   ├── agents-core/
│   │   ├── agents-llm/
│   │   ├── agents-server/
│   │   └── agents-workers/
│   └── Dockerfile
├── frontend/
│   ├── src/
│   │   ├── lib/stores/
│   │   └── routes/
│   ├── Dockerfile
│   └── Dockerfile.dev
├── docker-compose.yml
└── docker-compose.override.yml
```
