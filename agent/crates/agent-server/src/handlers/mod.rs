pub mod model;
pub mod pipeline;

pub async fn health() -> &'static str {
    "OK"
}
