//! HTTP route handlers for the agent server.

pub mod chat;
pub mod init;
pub mod model;
pub mod pipeline;
pub mod tools;
pub mod traces;

/// Health check endpoint.
pub async fn health() -> &'static str {
    "OK"
}
