use config::{Config, ConfigError};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct App {
    pub summary_size: usize,
    pub channel_capacity: usize,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Binance {
    pub currency_pair: String,
    pub depth: usize,
    pub latency: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Bitstamp {
    pub currency_pair: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Server {
    pub address: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub app: App,
    pub binance: Binance,
    pub bitstamp: Bitstamp,
    pub server: Server,
}

const CONFIG_FILE_PATH: &str = "./Settings.toml";

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let s = Config::builder()
            .add_source(config::File::with_name(CONFIG_FILE_PATH))
            .build()
            .unwrap();

        s.try_deserialize::<Self>()
    }
}
