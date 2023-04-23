mod place;
use std::net::{Ipv6Addr, IpAddr, SocketAddr};

use surge_ping::{AsyncSocket, Config};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = Config::new();
    // config.bind = Some(SocketAddr::new(ip, port));
    let socket = AsyncSocket::new(&config);

    Ok(())
}
