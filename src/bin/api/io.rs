use reqwest::Client;

use super::models::{EarthquakeFeature, UsgsResponse};

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
