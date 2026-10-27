use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::Html,
    routing::get,
};
use chrono::{Duration, Local, NaiveDate};
use tower_http::services::{ServeDir, ServeFile};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use super::{
    AppState,
    io::fetch_cached_earthquake_features,
    models::{ApiError, EarthquakesResponse, NearestEarthquakesQuery},
    transformations::{filter_earthquake_features, find_nearest_earthquakes},
};

/// Builds the HTTP router and attaches shared application state.
pub(super) fn build_router(state: AppState) -> Router {
    Router::new()
        .route_service("/", ServeFile::new("static/index.html"))
        .route("/welcome", get(read_root))
        .route("/earthquakes/near-me", get(get_nearest_earthquakes))
        .nest_service("/static", ServeDir::new("static"))
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .with_state(state)
}

/// Generates the OpenAPI specification consumed by Swagger UI.
///
/// Query parameters are documented through `params` on their route.
#[derive(OpenApi)]
#[openapi(
    paths(read_root, get_nearest_earthquakes),
    components(schemas(EarthquakesResponse, ApiError)),
    tags((
        name = "earthquakes",
        description = "Endpoints for finding nearby earthquakes from the USGS feed."
    )),
    info(
        title = "Earthquake API",
        version = "1.0.0",
        description = "An API for finding nearby earthquakes using current USGS GeoJSON data."
    )
)]
pub(super) struct ApiDoc;

/// Displays the API landing page.
#[utoipa::path(
    get,
    path = "/welcome",
    responses((
        status = 200,
        description = "HTML landing page",
        body = String,
        content_type = "text/html"
    ))
)]
async fn read_root() -> Html<String> {
    let current_datetime = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    Html(format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>Earthquake API</title>
    <style>
        body {{
            display: flex;
            justify-content: center;
            align-items: center;
            min-height: 100vh;
            margin: 0;
            font-family: Arial, sans-serif;
            background-color: #f0f8ff;
            color: #333;
            text-align: center;
        }}

        h1 {{
            font-size: 2.5em;
            margin-bottom: 0.5em;
        }}

        p {{
            font-size: 1.2em;
            margin: 0.5em 0;
        }}

        a {{
            color: #007bff;
            text-decoration: none;
        }}

        a:hover {{
            text-decoration: underline;
        }}
    </style>
</head>
<body>
    <div>
        <h1>Welcome to the Earthquake API!</h1>
        <p>Creator: Dilson Castro</p>
        <p>Current Date and Time: {current_datetime}</p>
        <p>
            To find earthquakes near you, navigate to
            <a href="/static/index.html">this link</a>.
        </p>
        <p>
            View and test API endpoints in
            <a href="/swagger-ui/">Swagger UI</a>.
        </p>
    </div>
</body>
</html>"#
    ))
}

/// Returns the closest current earthquakes to a supplied latitude and longitude.
#[utoipa::path(
    get,
    path = "/earthquakes/near-me",
    tag = "earthquakes",
    params(NearestEarthquakesQuery),
    responses(
        (
            status = 200,
            description = "Nearest earthquakes found",
            body = EarthquakesResponse,
            example = json!({
                "earthquakes": {
                    "M 2.5 - 10 km NW of Example Location": 15.0,
                    "M 3.1 - 20 km S of Another Location": 47.0
                }
            })
        ),
        (status = 400, description = "Invalid latitude, longitude, or result count", body = ApiError),
        (status = 502, description = "Could not retrieve valid data from USGS", body = ApiError)
    )
)]
async fn get_nearest_earthquakes(
    State(state): State<AppState>,
    Query(query): Query<NearestEarthquakesQuery>,
) -> Result<Json<EarthquakesResponse>, (StatusCode, Json<ApiError>)> {
    if !(-90.0..=90.0).contains(&query.lat) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: "Latitude must be between -90 and 90.".to_string(),
            }),
        ));
    }

    if !(-180.0..=180.0).contains(&query.lon) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: "Longitude must be between -180 and 180.".to_string(),
            }),
        ));
    }

    if query.num_earthquakes == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: "num_earthquakes must be greater than zero.".to_string(),
            }),
        ));
    }

    let date_window = parse_date_window(query.start_date.as_deref(), query.end_date.as_deref())
        .map_err(|message| (StatusCode::BAD_REQUEST, Json(ApiError { error: message })))?;

    let features =
        fetch_cached_earthquake_features(&state.client, &state.api_url, &state.feed_cache)
            .await
            .map_err(|message| (StatusCode::BAD_GATEWAY, Json(ApiError { error: message })))?;
    let earthquakes = filter_earthquake_features(&features, date_window);

    let locations =
        find_nearest_earthquakes(&earthquakes, query.lat, query.lon, query.num_earthquakes);
    let earthquakes = locations
        .iter()
        .map(|location| (location.title.clone(), location.distance_km as f64))
        .collect();

    Ok(Json(EarthquakesResponse {
        earthquakes,
        locations,
    }))
}

/// Parses an inclusive date range into UTC millisecond bounds.
fn parse_date_window(
    start_date: Option<&str>,
    end_date: Option<&str>,
) -> Result<Option<(i64, i64)>, String> {
    let (Some(start_date), Some(end_date)) = (start_date, end_date) else {
        return if start_date.is_none() && end_date.is_none() {
            Ok(None)
        } else {
            Err("Both start_date and end_date are required.".to_string())
        };
    };

    let start_date = NaiveDate::parse_from_str(start_date, "%Y-%m-%d")
        .map_err(|_| "Dates must use YYYY-MM-DD format.".to_string())?;
    let end_date = NaiveDate::parse_from_str(end_date, "%Y-%m-%d")
        .map_err(|_| "Dates must use YYYY-MM-DD format.".to_string())?;

    if start_date > end_date {
        return Err("start_date must be on or before end_date.".to_string());
    }

    let start_time = start_date
        .and_hms_opt(0, 0, 0)
        .expect("midnight is a valid time")
        .and_utc()
        .timestamp_millis();
    let end_time = end_date
        .checked_add_signed(Duration::days(1))
        .ok_or_else(|| "end_date is out of range.".to_string())?
        .and_hms_opt(0, 0, 0)
        .expect("midnight is a valid time")
        .and_utc()
        .timestamp_millis();

    Ok(Some((start_time, end_time)))
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use reqwest::Client;
    use serde_json::{Value, json};

    use super::*;
    use crate::api::io::FeedCache;

    /// Serves `router` on an ephemeral local port and returns its base URL.
    async fn spawn(router: Router) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        format!("http://{address}")
    }

    /// Starts a fake USGS feed that counts how often it is requested.
    async fn spawn_feed(status: StatusCode) -> (String, Arc<AtomicUsize>) {
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&hits);
        let feed = json!({
            "features": [
                {
                    "id": "nyc",
                    "properties": { "title": "M 2.0 - New York", "time": 1_700_000_000_000_i64 },
                    "geometry": { "coordinates": [-74.0060, 40.7128, 5.0] }
                },
                {
                    "id": "lisbon",
                    "properties": { "title": "M 3.0 - Lisbon", "time": 1_700_000_000_000_i64 },
                    "geometry": { "coordinates": [-9.1393, 38.7223, 5.0] }
                },
                {
                    "id": "tokyo",
                    "properties": { "title": "M 4.0 - Tokyo", "time": 1_700_000_000_000_i64 },
                    "geometry": { "coordinates": [139.6503, 35.6762, 5.0] }
                }
            ]
        });
        let router = Router::new().route(
            "/feed",
            get(move || {
                counter.fetch_add(1, Ordering::SeqCst);
                let feed = feed.clone();
                async move { (status, Json(feed)) }
            }),
        );
        (format!("{}/feed", spawn(router).await), hits)
    }

    async fn spawn_api(api_url: String, cache_ttl: Duration) -> String {
        spawn(build_router(AppState {
            client: Client::new(),
            api_url,
            feed_cache: Arc::new(FeedCache::new(cache_ttl)),
        }))
        .await
    }

    async fn get_json(url: String) -> (StatusCode, Value) {
        let response = reqwest::get(url).await.unwrap();
        let status = StatusCode::from_u16(response.status().as_u16()).unwrap();
        (status, response.json().await.unwrap())
    }

    fn titles(body: &Value) -> Vec<&str> {
        body["locations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|location| location["title"].as_str().unwrap())
            .collect()
    }

    #[tokio::test]
    async fn results_follow_the_requested_location_and_reuse_the_feed() {
        let (feed_url, hits) = spawn_feed(StatusCode::OK).await;
        let api = spawn_api(feed_url, Duration::from_secs(60)).await;

        let (status, from_new_york) = get_json(format!(
            "{api}/earthquakes/near-me?lat=40.7128&lon=-74.0060"
        ))
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            titles(&from_new_york),
            ["M 2.0 - New York", "M 3.0 - Lisbon", "M 4.0 - Tokyo"]
        );
        assert_eq!(from_new_york["locations"][0]["distance_km"], 0);
        assert_eq!(from_new_york["earthquakes"]["M 2.0 - New York"], 0.0);

        let (status, from_tokyo) = get_json(format!(
            "{api}/earthquakes/near-me?lat=35.6762&lon=139.6503&num_earthquakes=2"
        ))
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(titles(&from_tokyo), ["M 4.0 - Tokyo", "M 2.0 - New York"]);

        assert_eq!(hits.load(Ordering::SeqCst), 1, "feed should be cached");
    }

    #[tokio::test]
    async fn expired_feed_is_fetched_again() {
        let (feed_url, hits) = spawn_feed(StatusCode::OK).await;
        let api = spawn_api(feed_url, Duration::ZERO).await;

        for _ in 0..2 {
            let (status, _) = get_json(format!("{api}/earthquakes/near-me?lat=0&lon=0")).await;
            assert_eq!(status, StatusCode::OK);
        }
        assert_eq!(hits.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn date_filters_still_apply() {
        let (feed_url, _) = spawn_feed(StatusCode::OK).await;
        let api = spawn_api(feed_url, Duration::from_secs(60)).await;

        let (status, body) = get_json(format!(
            "{api}/earthquakes/near-me?lat=0&lon=0&start_date=2020-01-01&end_date=2020-01-02"
        ))
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(titles(&body).is_empty());
    }

    #[tokio::test]
    async fn invalid_coordinates_are_rejected_without_calling_the_feed() {
        let (feed_url, hits) = spawn_feed(StatusCode::OK).await;
        let api = spawn_api(feed_url, Duration::from_secs(60)).await;

        for query in [
            "lat=91&lon=0",
            "lat=-90.5&lon=0",
            "lat=0&lon=180.1",
            "lat=0&lon=0&num_earthquakes=0",
        ] {
            let (status, body) = get_json(format!("{api}/earthquakes/near-me?{query}")).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{query}");
            assert!(body["error"].is_string(), "{query}");
        }
        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn provider_failures_return_bad_gateway_and_are_not_cached() {
        let (feed_url, hits) = spawn_feed(StatusCode::INTERNAL_SERVER_ERROR).await;
        let api = spawn_api(feed_url, Duration::from_secs(60)).await;

        for _ in 0..2 {
            let (status, body) = get_json(format!("{api}/earthquakes/near-me?lat=0&lon=0")).await;
            assert_eq!(status, StatusCode::BAD_GATEWAY);
            assert_eq!(
                body["error"],
                "The earthquake data provider returned an error."
            );
        }
        assert_eq!(hits.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn date_window_is_inclusive_of_the_end_day() {
        assert_eq!(parse_date_window(None, None), Ok(None));
        assert_eq!(
            parse_date_window(Some("2024-01-01"), Some("2024-01-01")),
            Ok(Some((1_704_067_200_000, 1_704_153_600_000)))
        );
        assert!(parse_date_window(Some("2024-01-02"), Some("2024-01-01")).is_err());
        assert!(parse_date_window(Some("2024-01-01"), None).is_err());
        assert!(parse_date_window(Some("01/01/2024"), Some("2024-01-02")).is_err());
    }
}
