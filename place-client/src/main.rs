use futures::future;
use image::{GenericImageView, ImageFormat};
use std::{
    fs::File,
    io::BufReader,
    net::{IpAddr, Ipv6Addr},
    sync::Arc, time::Duration,
};
use surge_ping::{Client, Config, ICMP};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = Config::new();
    config.kind = ICMP::V6;
    let client = Client::new(&config).unwrap();

    let file = BufReader::new(File::open("based.png")?);
    let image = Arc::new(image::load(file, ImageFormat::Png)?);

    loop {
        let mut handles = Vec::new();

        for x in 0..512 {
            for y in 0..512 {
                let mut pinger = client
                    .pinger(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0)), 0.into())
                    .await;
                let image = Arc::clone(&image);
                let handle = tokio::spawn(async move {
                    let [r, g, b, _] = image.get_pixel(x, y).0;
                    let parsed = Ipv6Addr::new(
                        0x2602,
                        0xfa9b,
                        0x42,
                        0x1000 | x as u16,
                        0x0000 | y as u16,
                        r as u16,
                        g as u16,
                        b as u16, // ((x as f64 / 512.0) * 255.0) as u16,
                                  // ((y as f64 / 512.0) * 255.0) as u16,
                                  // 0xff,
                    );
                    pinger.host = parsed.into();
                    unsafe { pinger.send_ping(0.into(), &[1; 8]).await.unwrap_unchecked() };
                });

                handles.push(handle);

                std::thread::sleep(std::time::Duration::from_nanos(50))
            }
        }
        future::join_all(handles).await;
    }
}
