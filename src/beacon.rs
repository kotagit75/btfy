use geojson::{FeatureCollection, GeometryValue};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
};

use crate::{CONFIG, util::hash::Hashed};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Beacon {
    pub values: Vec<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BeaconKey {
    pub latest_block_hash: Hashed,
    pub timestamp: i64,
}

impl BeaconKey {
    pub fn new(latest_block_hash: &Hashed, timestamp: i64) -> Self {
        Self {
            latest_block_hash: *latest_block_hash,
            timestamp,
        }
    }
}

pub trait BeaconCache: Send + Sync {
    fn get(&self, key: &BeaconKey) -> Option<Beacon>;
    fn insert(&self, key: BeaconKey, beacon: Beacon);
}

#[derive(Default)]
pub struct InMemoryBeaconCache {
    inner: Mutex<HashMap<BeaconKey, Beacon>>,
}

impl InMemoryBeaconCache {
    pub fn new() -> Self {
        Self::default()
    }
}

impl BeaconCache for InMemoryBeaconCache {
    fn get(&self, key: &BeaconKey) -> Option<Beacon> {
        self.inner
            .lock()
            .expect("beacon cache lock poisoned")
            .get(key)
            .cloned()
    }

    fn insert(&self, key: BeaconKey, beacon: Beacon) {
        self.inner
            .lock()
            .expect("beacon cache lock poisoned")
            .insert(key, beacon);
    }
}

const LOCATIONS_GEOJSON: &str = include_str!("beacon/locations.geojson");
static LOCATIONS_LOCATIONS: LazyLock<Vec<geojson::Position>> = LazyLock::new(|| {
    let Ok(collection) = LOCATIONS_GEOJSON.parse::<FeatureCollection>() else {
        return Vec::new();
    };
    collection
        .features
        .iter()
        .flat_map(|feature| feature.geometry.clone())
        .flat_map(|geometry| match geometry.value {
            GeometryValue::Point { coordinates } => Some(coordinates),
            _ => None,
        })
        .collect()
});

const TEMPERATURE_SERVER_URL: &str = "http://localhost:8000/";

async fn get_temperature(lon: f64, lat: f64, timestamp: i64) -> Option<i32> {
    let result = reqwest::Client::new()
        .get(TEMPERATURE_SERVER_URL)
        .timeout(std::time::Duration::from_secs(CONFIG.args.beacon_timeout))
        .query(&[
            ("lat", lat.to_string()),
            ("lon", lon.to_string()),
            ("timestamp", timestamp.to_string()),
        ])
        .send()
        .await;
    match result {
        // Record the temperature as an integer by multiplying the value (up to one decimal places) by 10.
        Ok(res) => res
            .json::<f32>()
            .await
            .ok()
            .map(|t| (t * 10.0).round() as i32),
        Err(err) => {
            error!("failed to retrieve the temperature: {}", err);
            None
        }
    }
}

fn choose_locations(latest_block_hash: &Hashed) -> Vec<geojson::Position> {
    let len = LOCATIONS_LOCATIONS.len();
    if len == 0 {
        return Vec::new();
    }
    latest_block_hash
        .iter()
        .flat_map(|i| LOCATIONS_LOCATIONS.get((*i as usize) % len))
        .cloned()
        .collect()
}

pub async fn get_beacon(latest_block_hash: &Hashed, timestamp: i64) -> Option<Beacon> {
    let locations: Vec<geojson::Position> = choose_locations(latest_block_hash);
    info!("start getting beacon");
    let mut temperatures: Vec<i32> = Vec::new();
    for (i, pos) in locations.iter().enumerate() {
        info!("getting temperature for location {}", i);
        if let Some(temp) = get_temperature(pos[0], pos[1], timestamp).await {
            temperatures.push(temp);
        } else {
            error!("failed to get temperature for location {}", i);
            return None;
        }
    }
    if temperatures.len() != locations.len() {
        error!("failed to get beacon");
        return None;
    }
    info!("completed getting beacon");
    Some(Beacon {
        values: temperatures,
    })
}

pub async fn prefetch_beacon(
    cache: &dyn BeaconCache,
    latest_block_hash: &Hashed,
    timestamp: i64,
) -> bool {
    let key = BeaconKey::new(latest_block_hash, timestamp);
    if cache.get(&key).is_some() {
        return true;
    }
    let Some(beacon) = get_beacon(latest_block_hash, timestamp).await else {
        return false;
    };
    cache.insert(key, beacon);
    true
}

pub fn is_valid_beacon(own_beacon: &Beacon, target_beacon: &Beacon) -> bool {
    own_beacon
        .values
        .iter()
        .zip(target_beacon.values.iter())
        .all(
            |(a, b)| (a - b).abs() <= 5, /* Allowable error is within 0.5 degrees Celsius.*/
        )
}
