use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use reqwest::Client;
use tokio::sync::Mutex;

use super::models::{EarthquakeFeature, UsgsResponse};

/// Most recently fetched provider features, reused until they expire.
pub(super) struct FeedCache {
    ttl: Duration,
    entry: Mutex<Option<(Instant, Arc<Vec<EarthquakeFeature>>)>>,
}

impl FeedCache {
    /// Creates an empty cache whose entries stay fresh for `ttl`.
    pub(super) fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            entry: Mutex::new(None),
        }
    }
}

/// Returns cached earthquake features while fresh, otherwise fetches and caches them.
///
/// The lock is held while fetching so concurrent requests share one provider call.
/// Failed fetches are not cached.
pub(super) async fn fetch_cached_earthquake_features(
    client: &Client,
    api_url: &str,
    cache: &FeedCache,
) -> Result<Arc<Vec<EarthquakeFeature>>, String> {
    let mut entry = cache.entry.lock().await;
    if let Some((fetched_at, features)) = entry.as_ref() {
        if fetched_at.elapsed() < cache.ttl {
            return Ok(Arc::clone(features));
        }
    }

    let features = Arc::new(fetch_earthquake_features(client, api_url).await?);
    *entry = Some((Instant::now(), Arc::clone(&features)));
    Ok(features)
}

/// Fetches and deserializes earthquake features from the configured provider.
pub(super) async fn fetch_earthquake_features(
    client: &Client,
    api_url: &str,
) -> Result<Vec<EarthquakeFeature>, String> {
    let response = client
        .get(api_url)
        .send()
        .await
        .map_err(|error| {
            eprintln!("Error calling earthquake API: {error}");
            "Could not contact the earthquake data provider.".to_string()
        })?
        .error_for_status()
        .map_err(|error| {
            eprintln!("Earthquake API returned an error response: {error}");
            "The earthquake data provider returned an error.".to_string()
        })?;

    let data: UsgsResponse = response.json().await.map_err(|error| {
        eprintln!("Could not parse earthquake API JSON: {error}");
        "The earthquake data provider returned invalid JSON.".to_string()
    })?;

    Ok(data.features)
}
