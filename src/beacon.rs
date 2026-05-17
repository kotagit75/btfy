use geojson::{FeatureCollection, GeometryValue};
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::LazyLock;

use crate::util::{command::run_command_and_get_output, hash::Hashed};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Beacon {
    pub values: Vec<f32>,
}

const TEMPERATURE_SCRIPT_PATH: &str = "beacon/temperature";
const TARGET_GEOJSON: &str = include_str!("beacon/target.geojson");
static TARGET_LOCATIONS: LazyLock<Vec<geojson::Position>> = LazyLock::new(|| {
    let Ok(collection) = TARGET_GEOJSON.parse::<FeatureCollection>() else {
        return Vec::new();
    };
    collection
        .features
        .iter()
        .map(|feature| feature.geometry.clone())
        .flatten()
        .map(|geometry| match geometry.value {
            GeometryValue::Point { coordinates } => Some(coordinates),
            _ => None,
        })
        .flatten()
        .collect()
});

fn get_temperature(lon: f64, lat: f64) -> Option<f32> {
    let output = run_command_and_get_output(
        Command::new(TEMPERATURE_SCRIPT_PATH).args([lat.to_string(), lon.to_string()]),
    );
    let result = output.map(|str| str.parse::<f32>().ok()).flatten();
    if result.is_none() {
        error!("failed to retrieve the temperature");
    }
    result
}

fn choose_locations(latest_block_hash: &Hashed) -> Vec<geojson::Position> {
    let len = TARGET_LOCATIONS.len();
    latest_block_hash
        .iter()
        .map(|i| (*i as usize) % len)
        .map(|i| TARGET_LOCATIONS.get(i))
        .flatten()
        .cloned()
        .collect()
}

pub fn get_beacon(latest_block_hash: &Hashed) -> Option<Beacon> {
    let locations: Vec<geojson::Position> = choose_locations(latest_block_hash);
    info!("start getting beacon");
    let temperatures: Vec<_> = locations
        .iter()
        .map(|pos| get_temperature(pos[0], pos[1]))
        .collect();
    if temperatures.iter().any(|t| t.is_none()) {
        info!("failed to get beacon");
        return None;
    }
    info!("completed getting beacon");
    Some(Beacon {
        values: temperatures.iter().flatten().cloned().collect(),
    })
}

pub fn is_valid_beacon(target_beacon: &Beacon, latest_block_hash: &Hashed) -> bool {
    match get_beacon(latest_block_hash) {
        Some(beacon) => beacon
            .values
            .iter()
            .zip(target_beacon.values.iter())
            .all(|(a, b)| (a - b).abs() <= 0.5),
        None => false,
    }
}
