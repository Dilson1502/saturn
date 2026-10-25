mod constants;
mod io;
mod models;
mod routes;
mod server;
mod transformations;

use reqwest::Client;

/// Shared dependencies and configuration for API handlers.
#[derive(Clone)]
pub(super) struct AppState {
    pub(super) client: Client,
    pub(super) api_url: String,
}

/// Starts the API server.
pub(super) async fn run() -> Result<(), Box<dyn std::error::Error>> {
    server::serve().await
}
