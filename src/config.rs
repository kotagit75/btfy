use std::{env, sync::LazyLock};

pub struct Config {
    pub api_port: u16,
    pub cors_allow_port: u16,
}

pub static CONFIG: LazyLock<Config> = LazyLock::new(|| {
    let api_port = match env::var("API_PORT") {
        Ok(val) => val.parse().unwrap(),
        Err(_) => 8080,
    };
    let cors_allow_port = match env::var("CORS_ALLOW_PORT") {
        Ok(val) => val.parse().unwrap(),
        Err(_) => 3000,
    };
    Config {
        api_port,
        cors_allow_port,
    }
});
