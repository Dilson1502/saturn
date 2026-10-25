use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::constants::DEFAULT_NUMBER_OF_EARTHQUAKES;

/// Parameters accepted by the nearby-earthquakes endpoint.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(super) struct NearestEarthquakesQuery {
    /// Latitude of the location to search from. Valid range: -90 to 90.
    #[param(example = 40.7128)]
    pub(super) lat: f64,
    /// Longitude of the location to search from. Valid range: -180 to 180.
    #[param(example = -74.0060)]
    pub(super) lon: f64,
    /// Number of nearest earthquakes to return.
    #[serde(default = "default_number_of_earthquakes")]
    #[param(default = 10, example = 10)]
    pub(super) num_earthquakes: usize,
    /// Inclusive UTC start date in YYYY-MM-DD format.
    #[serde(default)]
    pub(super) start_date: Option<String>,
    /// Inclusive UTC end date in YYYY-MM-DD format.
    #[serde(default)]
    pub(super) end_date: Option<String>,
}

/// JSON response for a successful nearby-earthquake query.
#[derive(Debug, Serialize, ToSchema)]
pub(super) struct EarthquakesResponse {
    /// Key: earthquake title. Value: approximate distance in kilometers.
    pub(super) earthquakes: HashMap<String, f64>,
    /// Nearest earthquakes with coordinates for map rendering.
    pub(super) locations: Vec<NearbyEarthquake>,
}

/// One nearby earthquake with its location and rounded distance.
#[derive(Debug, Serialize, ToSchema)]
pub(super) struct NearbyEarthquake {
    pub(super) title: String,
    pub(super) latitude: f64,
    pub(super) longitude: f64,
    pub(super) distance_km: u64,
    pub(super) time: Option<i64>,
}

/// Standard JSON error response.
#[derive(Debug, Serialize, ToSchema)]
pub(super) struct ApiError {
    pub(super) error: String,
}

/// Structure returned by the USGS GeoJSON feed.
#[derive(Debug, Deserialize)]
pub(super) struct UsgsResponse {
    #[serde(default)]
    pub(super) features: Vec<EarthquakeFeature>,
}

/// One earthquake item returned by the USGS feed.
#[derive(Debug, Deserialize)]
pub(super) struct EarthquakeFeature {
    pub(super) id: Option<String>,
    pub(super) properties: Option<EarthquakeProperties>,
    pub(super) geometry: Option<EarthquakeGeometry>,
}

/// Descriptive properties returned for an earthquake.
#[derive(Debug, Deserialize)]
pub(super) struct EarthquakeProperties {
    pub(super) title: Option<String>,
    pub(super) time: Option<i64>,
}

/// Geographic data returned for an earthquake.
#[derive(Debug, Deserialize)]
pub(super) struct EarthquakeGeometry {
    /// USGS GeoJSON coordinates are [longitude, latitude, depth].
    pub(super) coordinates: Vec<f64>,
}

/// Internal, cleaned-up earthquake representation.
#[derive(Debug)]
pub(super) struct Earthquake {
    pub(super) title: String,
    pub(super) latitude: f64,
    pub(super) longitude: f64,
    pub(super) time: Option<i64>,
}

/// Supplies the default result count when the query omits it.
fn default_number_of_earthquakes() -> usize {
    DEFAULT_NUMBER_OF_EARTHQUAKES
}
