pub(super) const DEFAULT_NUMBER_OF_EARTHQUAKES: usize = 10;
pub(super) const EARTH_RADIUS_KM: f64 = 6_371.0;
pub(super) const DEFAULT_PORT: u16 = 5005;
pub(super) const HTTP_TIMEOUT_SECONDS: u64 = 10;
/// How long a fetched USGS feed is reused before it is requested again.
pub(super) const FEED_CACHE_TTL_SECONDS: u64 = 60;
