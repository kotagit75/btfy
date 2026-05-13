use geojson::{FeatureCollection, GeometryValue};
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::LazyLock;

use crate::util::{command::run_command_and_get_output, hash::Hashed};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Beacon {
    pub value: f32,
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

fn choose_locations(lastest_block_hash: &Hashed) -> Vec<geojson::Position> {
    let len = TARGET_LOCATIONS.len();
    lastest_block_hash
        .iter()
        .map(|i| (*i as usize) % len)
        .map(|i| TARGET_LOCATIONS.get(i))
        .flatten()
        .cloned()
        .collect()
}

pub fn get_beacon(lastest_block_hash: &Hashed) -> Option<Beacon> {
    let locations: Vec<geojson::Position> = choose_locations(lastest_block_hash);
    info!("start getting beacon");
    let sum: f32 = locations
        .iter()
        .map(|pos| get_temperature(pos[0], pos[1]))
        .flatten()
        .sum();
    info!("completed getting beacon");
    Some(Beacon { value: sum })
}

pub fn is_valid_beacon(target_beacon: &Beacon, lastest_block_hash: &Hashed) -> bool {
    match get_beacon(lastest_block_hash) {
        Some(beacon) => (beacon.value - target_beacon.value).abs() <= 0.5,
        None => false,
    }
}
