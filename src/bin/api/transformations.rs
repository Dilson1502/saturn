use super::{
    constants::EARTH_RADIUS_KM,
    models::{Earthquake, EarthquakeFeature, NearbyEarthquake},
};

/// Converts provider features into validated internal earthquake records.
pub(super) fn filter_earthquake_features(
    features: Vec<EarthquakeFeature>,
    date_window: Option<(i64, i64)>,
) -> Vec<Earthquake> {
    features
        .into_iter()
        .filter_map(|feature| {
            feature.id?;

            let properties = feature.properties?;
            let geometry = feature.geometry?;

            if let Some((start_time, end_time)) = date_window {
                let event_time = properties.time?;
                if !(start_time..end_time).contains(&event_time) {
                    return None;
                }
            }

            if geometry.coordinates.len() < 2 {
                return None;
            }

            Some(Earthquake {
                title: properties
                    .title
                    .unwrap_or_else(|| "Unknown earthquake".to_string()),
                longitude: geometry.coordinates[0],
                latitude: geometry.coordinates[1],
                time: properties.time,
            })
        })
        .collect()
}

/// Finds the nearest earthquakes and returns at most the requested count.
pub(super) fn find_nearest_earthquakes(
    earthquakes: &[Earthquake],
    input_latitude: f64,
    input_longitude: f64,
    number_of_earthquakes: usize,
) -> Vec<NearbyEarthquake> {
    let mut distances: Vec<(&Earthquake, u64)> = earthquakes
        .iter()
        .map(|earthquake| {
            let distance_km = calculate_haversine_distance_km(
                input_latitude,
                input_longitude,
                earthquake.latitude,
                earthquake.longitude,
            );
            (earthquake, distance_km.floor() as u64)
        })
        .collect();

    distances.sort_by_key(|(_, distance)| *distance);

    distances
        .into_iter()
        .take(number_of_earthquakes)
        .map(|(earthquake, distance)| NearbyEarthquake {
            title: earthquake.title.clone(),
            latitude: earthquake.latitude,
            longitude: earthquake.longitude,
            distance_km: distance,
            time: earthquake.time,
        })
        .collect()
}

/// Calculates great-circle distance in kilometers using the Haversine formula.
fn calculate_haversine_distance_km(
    latitude_1: f64,
    longitude_1: f64,
    latitude_2: f64,
    longitude_2: f64,
) -> f64 {
    let latitude_1 = latitude_1.to_radians();
    let longitude_1 = longitude_1.to_radians();
    let latitude_2 = latitude_2.to_radians();
    let longitude_2 = longitude_2.to_radians();

    let delta_latitude = latitude_2 - latitude_1;
    let delta_longitude = longitude_2 - longitude_1;

    let a = (delta_latitude / 2.0).sin().powi(2)
        + latitude_1.cos() * latitude_2.cos() * (delta_longitude / 2.0).sin().powi(2);
    let central_angle = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

    EARTH_RADIUS_KM * central_angle
}
