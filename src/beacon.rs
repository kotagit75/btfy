use bitcode::{Decode, Encode};
use geojson::{FeatureCollection, GeometryValue};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    process::Stdio,
    sync::{LazyLock, Mutex},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::Mutex as AsyncMutex,
    time::timeout,
};

use crate::{
    CONFIG,
    util::{hash::Hashed, progressbar::get_progress_bar},
};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Encode, Decode)]
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

#[derive(Debug, Deserialize)]
struct BeaconResponse {
    temperature: i32,
}

struct BeaconProcess {
    _child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl BeaconProcess {
    fn spawn() -> Option<Self> {
        let command = &CONFIG.args.beacon_cmd;
        if command.is_empty() {
            error!("beacon command is not configured");
            return None;
        }

        let mut child = Command::new(&command[0]);
        child
            .args(&command[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        let mut child = match child.spawn() {
            Ok(child) => child,
            Err(err) => {
                error!("failed to start beacon process: {}", err);
                return None;
            }
        };

        let Some(stdin) = child.stdin.take() else {
            error!("failed to open beacon process stdin");
            return None;
        };
        let Some(stdout) = child.stdout.take() else {
            error!("failed to open beacon process stdout");
            return None;
        };

        Some(Self {
            _child: child,
            stdin,
            stdout: BufReader::new(stdout),
        })
    }

    async fn get_temperature(&mut self, lat: f64, lon: f64, timestamp: i64) -> Option<i32> {
        let timeout_duration = std::time::Duration::from_secs(CONFIG.args.beacon_timeout);

        timeout(timeout_duration, async {
            self.stdin
                .write_all(format!("{} {} {}\n", lat, lon, timestamp).as_bytes())
                .await
                .ok()?;
            self.stdin.flush().await.ok()?;

            let mut line = String::new();
            let read = self.stdout.read_line(&mut line).await.ok()?;
            if read == 0 {
                return None;
            }

            serde_json::from_str::<BeaconResponse>(line.trim())
                .ok()
                .map(|r| r.temperature)
        })
        .await
        .ok()
        .flatten()
    }
}

static BEACON_PROCESS: LazyLock<AsyncMutex<Option<BeaconProcess>>> =
    LazyLock::new(|| AsyncMutex::new(None));

async fn get_temperature(lat: f64, lon: f64, timestamp: i64) -> Option<i32> {
    let mut guard = BEACON_PROCESS.lock().await;
    if guard.is_none() {
        *guard = BeaconProcess::spawn();
    }
    let result = match guard.as_mut() {
        Some(process) => process.get_temperature(lat, lon, timestamp).await,
        None => None,
    };
    if result.is_none() {
        *guard = None;
    }
    result
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

    let pb = get_progress_bar(locations.len() as u64);

    for (i, pos) in locations.iter().enumerate() {
        if let Some(temp) = get_temperature(pos[1], pos[0], timestamp).await {
            temperatures.push(temp);
            pb.inc(1);
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
