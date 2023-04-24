use std::net::Ipv6Addr;

use config::{Config, ConfigError};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Settings {
    prefix48: Ipv6Addr,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let settings = Config::builder()
            .add_source(config::File::with_name("config.toml"))
            .add_source(config::Environment::with_prefix("V6PLACE_"))
            .build()?;

        settings.try_deserialize()
    }
}
