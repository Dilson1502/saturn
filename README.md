# Saturn – Earthquake API

Rust (axum) API that returns the closest current earthquakes to a given latitude/longitude, using the [USGS GeoJSON feed](https://earthquake.usgs.gov/earthquakes/feed/v1.0/geojson.php). Includes a small web UI and Swagger docs.

The web UI starts from your GPS location; drag the marker or tap the map to search around any other point, and use **Return to my location** to go back. The USGS feed is cached in memory for 60 seconds, so moving the marker does not re-download it.

## Development

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
node --test "tests/frontend/*.test.js"   # web UI location-state tests (Node 20+)
```