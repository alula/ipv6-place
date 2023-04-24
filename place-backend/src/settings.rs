use std::net::Ipv6Addr;

use config::{Config, ConfigError};
use serde::Deserialize;

use crate::utils::{Color, RangedU16};

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub backend: BackendSettings,
    pub canvas: CanvasSettings,
    pub websocket: WebSocketSettings,
}

#[derive(Debug, Deserialize)]
pub struct CanvasSettings {
    /// Size of the canvas in pixels. Acceptable values are 16-4096, default is 512.
    #[serde(default = "CanvasSettings::default_size")]
    pub size: RangedU16<16, 4096>,
    /// The background color of the canvas in form of "#rrggbb" string, default is "#ffffff".
    #[serde(default = "CanvasSettings::default_background_color")]
    pub background_color: Color,
    /// The filename to save the canvas to, default is "place.png".
    #[serde(default = "CanvasSettings::default_filename")]
    pub filename: String,
}

impl CanvasSettings {
    fn default_size() -> RangedU16<16, 4096> {
        RangedU16::new(512).unwrap()
    }

    fn default_background_color() -> Color {
        Color::rgb(255, 255, 255)
    }

    fn default_filename() -> String {
        "place.png".to_string()
    }
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BackendType {
    Smoltcp,
}

#[derive(Debug, Deserialize)]
pub struct BackendSettings {
    /// A /48 IPv6 prefix to listen for pings on.
    pub prefix48: Ipv6Addr,

    /// The backend to use. Available options are: "smoltcp".
    pub backend_type: BackendType,

    pub smoltcp: SmoltcpSettings,
}

#[derive(Debug, Deserialize)]
pub struct SmoltcpSettings {
    /// Name of TAP interface to use.
    pub tap_iface: String,
}

#[derive(Debug, Deserialize)]
pub struct WebSocketSettings {
    /// Listening address:port for the WebSocket server, default is "[::]:2137".
    #[serde(default = "WebSocketSettings::default_listen_addr")]
    pub listen_addr: String,
}

impl WebSocketSettings {
    fn default_listen_addr() -> String {
        "[::]:2137".to_string()
    }
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let settings = Config::builder()
            .add_source(config::File::with_name("config.toml"))
            .add_source(config::Environment::with_prefix("PLACE_"))
            .build()?;

        settings.try_deserialize()
    }
}
