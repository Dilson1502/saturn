use super::{
    constants::EARTH_RADIUS_KM,
    models::{Earthquake, EarthquakeFeature, NearbyEarthquake},
};

/// Converts provider features into validated internal earthquake records.
pub(super) fn filter_earthquake_features(
    features: &[EarthquakeFeature],
    date_window: Option<(i64, i64)>,
) -> Vec<Earthquake> {
    features
        .iter()
        .filter_map(|feature| {
            feature.id.as_ref()?;

            let properties = feature.properties.as_ref()?;
            let geometry = feature.geometry.as_ref()?;

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
                    .clone()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::models::{EarthquakeGeometry, EarthquakeProperties};

    fn feature(id: &str, latitude: f64, longitude: f64, time: Option<i64>) -> EarthquakeFeature {
        EarthquakeFeature {
            id: Some(id.to_string()),
            properties: Some(EarthquakeProperties {
                title: Some(format!("Quake {id}")),
                time,
            }),
            geometry: Some(EarthquakeGeometry {
                coordinates: vec![longitude, latitude, 10.0],
            }),
        }
    }

    fn sample_earthquakes() -> Vec<Earthquake> {
        filter_earthquake_features(
            &[
                feature("equator", 0.0, 0.0, Some(1_000)),
                feature("new-york", 40.7128, -74.0060, Some(2_000)),
                feature("tokyo", 35.6762, 139.6503, Some(3_000)),
                feature("lisbon", 38.7223, -9.1393, Some(4_000)),
            ],
            None,
        )
    }

    fn titles(nearby: &[NearbyEarthquake]) -> Vec<&str> {
        nearby.iter().map(|quake| quake.title.as_str()).collect()
    }

    #[test]
    fn haversine_matches_known_distances() {
        assert_eq!(calculate_haversine_distance_km(10.0, 20.0, 10.0, 20.0), 0.0);
        // One degree of latitude is roughly 111.19 km on a sphere of radius 6371 km.
        let one_degree = calculate_haversine_distance_km(0.0, 0.0, 1.0, 0.0);
        assert!((one_degree - 111.19).abs() < 0.01, "got {one_degree}");
        // New York to London is about 5570 km.
        let transatlantic = calculate_haversine_distance_km(40.7128, -74.0060, 51.5074, -0.1278);
        assert!(
            (transatlantic - 5_570.0).abs() < 10.0,
            "got {transatlantic}"
        );
    }

    #[test]
    fn distances_are_measured_from_the_given_point_and_sorted() {
        let earthquakes = sample_earthquakes();

        let from_new_york = find_nearest_earthquakes(&earthquakes, 40.7128, -74.0060, 10);
        assert_eq!(
            titles(&from_new_york),
            [
                "Quake new-york",
                "Quake lisbon",
                "Quake equator",
                "Quake tokyo"
            ]
        );
        assert_eq!(from_new_york[0].distance_km, 0);

        let from_tokyo = find_nearest_earthquakes(&earthquakes, 35.6762, 139.6503, 10);
        assert_eq!(titles(&from_tokyo)[0], "Quake tokyo");
        assert_eq!(from_tokyo[0].distance_km, 0);

        for nearby in [&from_new_york, &from_tokyo] {
            assert!(
                nearby
                    .windows(2)
                    .all(|pair| pair[0].distance_km <= pair[1].distance_km)
            );
        }
    }

    #[test]
    fn moving_the_reference_point_changes_distances() {
        let earthquakes = sample_earthquakes();
        let lisbon_from_new_york = find_nearest_earthquakes(&earthquakes, 40.7128, -74.0060, 10)
            .into_iter()
            .find(|quake| quake.title == "Quake lisbon")
            .unwrap();
        let lisbon_from_equator = find_nearest_earthquakes(&earthquakes, 0.0, 0.0, 10)
            .into_iter()
            .find(|quake| quake.title == "Quake lisbon")
            .unwrap();

        let expected = calculate_haversine_distance_km(40.7128, -74.0060, 38.7223, -9.1393);
        assert_eq!(lisbon_from_new_york.distance_km, expected.floor() as u64);
        assert_ne!(
            lisbon_from_new_york.distance_km,
            lisbon_from_equator.distance_km
        );
    }

    #[test]
    fn result_count_is_limited() {
        let earthquakes = sample_earthquakes();
        let nearby = find_nearest_earthquakes(&earthquakes, 0.0, 0.0, 2);
        assert_eq!(titles(&nearby), ["Quake equator", "Quake lisbon"]);
    }

    #[test]
    fn filtering_drops_incomplete_features_and_applies_the_date_window() {
        let mut missing_id = feature("missing-id", 1.0, 1.0, Some(1_500));
        missing_id.id = None;
        let mut short_coordinates = feature("short", 1.0, 1.0, Some(1_500));
        short_coordinates.geometry = Some(EarthquakeGeometry {
            coordinates: vec![1.0],
        });
        let features = [
            feature("early", 1.0, 1.0, Some(999)),
            feature("start", 1.0, 1.0, Some(1_000)),
            feature("untimed", 1.0, 1.0, None),
            feature("end", 1.0, 1.0, Some(2_000)),
            missing_id,
            short_coordinates,
        ];

        let all: Vec<_> = filter_earthquake_features(&features, None)
            .into_iter()
            .map(|quake| quake.title)
            .collect();
        assert_eq!(
            all,
            ["Quake early", "Quake start", "Quake untimed", "Quake end"]
        );

        let windowed: Vec<_> = filter_earthquake_features(&features, Some((1_000, 2_000)))
            .into_iter()
            .map(|quake| quake.title)
            .collect();
        assert_eq!(windowed, ["Quake start"]);
    }
}
