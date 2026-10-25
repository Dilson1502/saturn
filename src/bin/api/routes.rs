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
    io::fetch_earthquake_features,
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

    let features = fetch_earthquake_features(&state.client, &state.api_url)
        .await
        .map_err(|message| (StatusCode::BAD_GATEWAY, Json(ApiError { error: message })))?;
    let earthquakes = filter_earthquake_features(features, date_window);

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
