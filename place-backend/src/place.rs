use image::{ImageFormat, Rgba, RgbaImage};
use tokio::sync::{RwLock, RwLockReadGuard};
use std::{
    fs::File,
    io::BufReader,
    path::{Path, PathBuf}, sync::Arc,
};

pub struct SharedImageHandle {
    data: Arc<RwLock<RgbaImage>>,
}

impl SharedImageHandle {
    pub fn new(data: RgbaImage) -> SharedImageHandle {
        SharedImageHandle {
            data: Arc::new(RwLock::new(data)),
        }
    }

    pub async fn put(&self, x: u32, y: u32, colour: u32) {
        let mut image = self.data.write().await;
        image[(x, y)] = Rgba(colour.to_le_bytes());
    }

    pub fn put_blocking(&self, x: u32, y: u32, colour: u32) {
        let mut image = self.data.blocking_write();
        image[(x, y)] = Rgba(colour.to_le_bytes());
    }

    pub async fn get_dimensions(&self) -> (u32, u32) {
        let image = self.data.read().await;
        image.dimensions()
    }

    pub fn get_dimensions_blocking(&self) -> (u32, u32) {
        let image = self.data.blocking_read();
        image.dimensions()
    }

    pub async fn get_image(&self) -> RwLockReadGuard<'_, RgbaImage> {
        self.data.read().await
    }

    pub fn get_image_blocking(&self) -> RwLockReadGuard<'_, RgbaImage> {
        self.data.blocking_read()
    }
}

impl Clone for SharedImageHandle {
    fn clone(&self) -> Self {
        SharedImageHandle {
            data: Arc::clone(&self.data),
        }
    }
}

pub struct Place {
    pub image: SharedImageHandle,
    pub path: PathBuf,
}

impl Place {
    pub fn new(path: impl AsRef<Path>) -> Result<Place, Box<dyn std::error::Error>> {
        let data = if path.as_ref().exists() {
            let f = File::open(path.as_ref())?;
            let image = BufReader::new(f);
            image::load(image, ImageFormat::Png)?.into_rgba8()
        } else {
            let data = RgbaImage::new(512, 512);
            data.save(path.as_ref())?;
            data
        };

        Ok(Place {
            image: SharedImageHandle::new(data),
            path: path.as_ref().to_owned(),
        })
    }

    pub fn put(&mut self, x: u32, y: u32, colour: u32) {
        self.image.put_blocking(x, y, colour);
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let image = self.image.get_image_blocking();
        image.save(&self.path)?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use futures::future;
    use std::{
        net::{IpAddr, Ipv6Addr},
    };
    use surge_ping::{Client, Config, ICMP};

    use super::*;

    #[test]
    fn nyauwunyanyanyanya() {
        let place = Place::new("test.png").unwrap();

        let th = 10;
        let (x, y) = place.image.get_dimensions_blocking();
        for x in 0..x {
            for y in 0..y {
                place.image.put_blocking(
                    x,
                    y,
                    u32::from_le_bytes([
                        ((x as f64 / 512.0) * 255.0) as u8,
                        ((y as f64 / 512.0) * 255.0) as u8,
                        // ((!((x & y) * (x | y)) as f64 / 512.0) * 255.0) as u8,
                        ((!((x & y) * (x | y)) as f64 / 512.0) * 255.0) as u8,
                        255,
                    ]),
                );
                // if x == y {
                //     place.put(x, y, 0xffffffff);
                //     (1..=th).for_each(|i| {
                //         place.put((x + i).min(511), y, 0xffffffff)
                //     });
                //     (1..=th).for_each(|i| {
                //         place.put(x, (y + i).min(511), 0xffffffff);
                //     });
                // }
            }
        }

        place.save().unwrap();
    }

    #[tokio::test]
    async fn ip6_wuuwuwuw() {
        let mut config = Config::new();
        config.kind = ICMP::V6;
        let client = Client::new(&config).unwrap();
        let mut handles = Vec::new();

        let passes = 3;
        for _ in 0..passes {
            for x in 0..512 {
                for y in 0..512 {
                    let mut pinger = client
                        .pinger(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0)), 0.into())
                        .await;
                    let handle = tokio::spawn(async move {
                        let parsed =
                            Ipv6Addr::new(
                                0x2602,0xfa9b, 0x42,
                                0x1000 | x, y, 
                                ((x as f64 / 512.0) * 255.0) as u16,
                                ((y as f64 / 512.0) * 255.0) as u16,
                                if y % 2 == 0 { 0x00 } else { 0xff }
                            );
                        pinger.host = parsed.into();
                        pinger.ping(0.into(), &[1; 8]).await.unwrap_err();
                    });

                    handles.push(handle);

                    // std::thread::sleep(std::time::Duration::from_nanos(500))
                }
            }
        }

        dbg!(handles.len());
        future::join_all(handles).await;
    }
}
