use std::net::Ipv6Addr;

use config::Config;
use serde::Deserialize;

use crate::{
    utils::{Color, RangedU16},
    PResult,
};

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

    /// Settings for the smoltcp backend.
    pub smoltcp: SmoltcpSettings,
}

#[derive(Debug, Deserialize)]
pub struct SmoltcpSettings {
    /// Name of TAP interface to use. Default is "tun0".
    #[serde(default = "SmoltcpSettings::default_tun_iface")]
    pub tun_iface: String,

    /// Size of receive buffer (in number of packets). Default is 65536.
    #[serde(default = "SmoltcpSettings::default_recv_buffer_size")]
    pub recv_buffer_size: usize,
}

impl SmoltcpSettings {
    fn default_tun_iface() -> String {
        "tun0".to_string()
    }

    fn default_recv_buffer_size() -> usize {
        65536
    }
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
    pub fn new() -> PResult<Self> {
        let settings = Config::builder()
            .add_source(config::File::with_name("config.toml"))
            .add_source(config::Environment::with_prefix("PLACE_"))
            .build()?;

        let settings = settings.try_deserialize::<Settings>()?;
        settings.sanity_check()?;
        Ok(settings)
    }

    fn sanity_check(&self) -> PResult<()> {
        let addr = self.backend.prefix48.segments();
        if addr[3..].iter().any(|&v| v != 0) {
            return Err("The specified /48 prefix must have it's lower bits set to 0.".into());
        }

        Ok(())
    }
}
