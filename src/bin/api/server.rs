use std::{env, sync::Arc, time::Duration};

use axum::http::Method;
use dotenvy::dotenv;
use reqwest::Client;
use tower_http::cors::{Any, CorsLayer};

use super::{
    AppState,
    constants::{DEFAULT_PORT, FEED_CACHE_TTL_SECONDS, HTTP_TIMEOUT_SECONDS},
    io::FeedCache,
    routes::build_router,
};

/// Loads configuration, builds the application, and serves HTTP requests.
pub(super) async fn serve() -> Result<(), Box<dyn std::error::Error>> {
    match dotenv() {
        Ok(_) => println!(".env found; environment variables loaded."),
        Err(_) => println!(".env not found; using system environment variables."),
    }

    let api_url = env::var("API_URL")
        .expect("API_URL is required. Add it to .env or export it as an environment variable.");
    let client = Client::builder()
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECONDS))
        .build()?;
    // Railway (and most PaaS hosts) inject the listening port through PORT.
    let port = match env::var("PORT") {
        Ok(value) => value
            .parse::<u16>()
            .map_err(|_| format!("PORT must be a valid port number, got '{value}'."))?,
        Err(_) => DEFAULT_PORT,
    };
    let address = format!("0.0.0.0:{port}");
    let state = AppState {
        client,
        api_url,
        feed_cache: Arc::new(FeedCache::new(Duration::from_secs(FEED_CACHE_TTL_SECONDS))),
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers(Any);
    let app = build_router(state).layer(cors);
    let listener = tokio::net::TcpListener::bind(&address).await?;

    println!("Server:     http://{address}");
    println!("Swagger UI: http://{address}/swagger-ui/");
    println!("OpenAPI:    http://{address}/api-docs/openapi.json");

    axum::serve(listener, app).await?;
    Ok(())
}
