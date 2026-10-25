#[path = "api/mod.rs"]
mod api;

/// Starts the Earthquake API server.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    api::run().await
}
