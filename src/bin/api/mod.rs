mod constants;
mod io;
mod models;
mod routes;
mod server;
mod transformations;

use std::sync::Arc;

use reqwest::Client;

use io::FeedCache;

/// Shared dependencies and configuration for API handlers.
#[derive(Clone)]
pub(super) struct AppState {
    pub(super) client: Client,
    pub(super) api_url: String,
    // Private: `FeedCache` is internal to `api`, whose child modules can still read this.
    feed_cache: Arc<FeedCache>,
}

/// Starts the API server.
pub(super) async fn run() -> Result<(), Box<dyn std::error::Error>> {
    server::serve().await
}
