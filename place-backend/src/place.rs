use image::{ImageFormat, RgbaImage};
use std::{fs::File, io::BufReader, path::PathBuf, sync::Arc};
use tokio::{
    sync::{broadcast, RwLock, RwLockReadGuard},
    task::JoinHandle,
};

use crate::{settings::CanvasSettings, utils::Color, PResult};

pub struct SharedImageHandle {
    data: Arc<RwLock<RgbaImage>>,
}

impl SharedImageHandle {
    pub fn new(data: RgbaImage) -> SharedImageHandle {
        SharedImageHandle {
            data: Arc::new(RwLock::new(data)),
        }
    }

    pub async fn put(&self, x: u32, y: u32, color: Color, big: bool) {
        let mut image = self.data.write().await;
        // let image = unsafe { &mut *self.data.get() };
        if (x, y) >= image.dimensions() {
            return;
        }

        image[(x, y)] = color.into_rgba();
        if big {
            image[(x + 1, y)] = color.into_rgba();
            image[(x, y + 1)] = color.into_rgba();
            image[(x + 1, y + 1)] = color.into_rgba();
        }
    }

    pub fn put_blocking(&self, x: u32, y: u32, color: Color, big: bool) {
        let mut image = self.data.blocking_write();
        // let image = unsafe { &mut *self.data.get() };
        if (x, y) >= image.dimensions() {
            return;
        }

        image[(x, y)] = color.into_rgba();
        if big {
            image[(x + 1, y)] = color.into_rgba();
            image[(x, y + 1)] = color.into_rgba();
            image[(x + 1, y + 1)] = color.into_rgba();
        }
    }

    pub async fn get_dimensions(&self) -> (u32, u32) {
        let image = self.data.read().await;
        // let image = unsafe { &mut *self.data.get() };
        image.dimensions()
    }

    pub fn get_dimensions_blocking(&self) -> (u32, u32) {
        let image = self.data.blocking_read();
        // let image = unsafe { &mut *self.data.get() };
        image.dimensions()
    }

    pub async fn get_image(&self) -> RwLockReadGuard<'_, RgbaImage> {
        self.data.read().await
    }

    pub fn get_image_blocking(&self) -> RwLockReadGuard<'_, RgbaImage> {
        self.data.blocking_read()
    }

    // pub async fn get_image(&self) -> &RgbaImage {
    //     let image = unsafe { &mut *self.data.get() };
    //     image
    // }

    // pub fn get_image_blocking(&self) -> &RgbaImage {
    //     let image = unsafe { &mut *self.data.get() };
    //     image
    // }
}

unsafe impl Send for SharedImageHandle {}
unsafe impl Sync for SharedImageHandle {}

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
    pub png_sender: broadcast::Sender<Arc<[u8]>>,
}

impl Place {
    pub fn new(settings: &CanvasSettings) -> PResult<Place> {
        if settings.filename.is_empty() {
            return Err("Filename must be set".into());
        }

        let path = PathBuf::from(&settings.filename);
        let size = settings.size.get() as u32;

        let data = if path.exists() {
            let f = File::open(&path)?;
            let image = BufReader::new(f);
            let image = image::load(image, ImageFormat::Png)?.into_rgba8();
            if image.dimensions() != (size, size) {
                return Err(format!(
                    "Image dimensions do not match configured canvas size: {:?} != {:?}",
                    image.dimensions(),
                    (size, size)
                )
                .into());
            }
            image
        } else {
            let mut data = RgbaImage::new(size, size);
            for pixel in data.pixels_mut() {
                *pixel = settings.background_color.into_rgba();
            }
            data.save(&path)?;
            data
        };

        let (png_sender, _) = broadcast::channel(8);

        Ok(Place {
            image: SharedImageHandle::new(data),
            path,
            png_sender,
        })
    }

    pub fn new_memory(settings: &CanvasSettings) -> PResult<Place> {
        let size = settings.size.get() as u32;

        let data = {
            let mut data = RgbaImage::new(size, size);
            for pixel in data.pixels_mut() {
                *pixel = settings.background_color.into_rgba();
            }
            data
        };

        let (png_sender, _) = broadcast::channel(8);

        Ok(Place {
            image: SharedImageHandle::new(data),
            path: PathBuf::from(""),
            png_sender,
        })
    }

    pub fn save(&self) -> PResult<()> {
        if self.path == PathBuf::from("") {
            return Err("No path to save to".into());
        }

        let image = self.image.get_image_blocking();
        image.save(&self.path)?;
        Ok(())
    }

    async fn diffing_task(
        image: SharedImageHandle,
        png_sender: broadcast::Sender<Arc<[u8]>>,
    ) -> PResult<()> {
        Ok(())
    }

    pub fn start_diffing_task(&self) -> JoinHandle<PResult<()>> {
        let image = self.image.clone();
        let png_sender = self.png_sender.clone();
        tokio::spawn(async move { Self::diffing_task(image, png_sender).await })
    }
}

#[cfg(test)]
mod test {
    use futures::future;
    use std::net::{IpAddr, Ipv6Addr};
    use surge_ping::{Client, Config, ICMP};

    use crate::utils::{Color, RangedU16};

    use super::*;

    #[test]
    fn nyauwunyanyanyanya() {
        let place = Place::new_memory(&CanvasSettings {
            size: RangedU16::new(512).unwrap(),
            background_color: Color::rgb(255, 255, 255),
            filename: String::new(),
        })
        .unwrap();

        let th = 10;
        let (x, y) = place.image.get_dimensions_blocking();
        for x in 0..x {
            for y in 0..y {
                place.image.put_blocking(
                    x,
                    y,
                    Color::new(
                        ((x as f64 / 512.0) * 255.0) as u8,
                        ((y as f64 / 512.0) * 255.0) as u8,
                        // ((!((x & y) * (x | y)) as f64 / 512.0) * 255.0) as u8,
                        ((!((x & y) * (x | y)) as f64 / 512.0) * 255.0) as u8,
                        255,
                    ),
                    false,
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
                        let parsed = Ipv6Addr::new(
                            0x2602,
                            0xfa9b,
                            0x42,
                            0x1000 | x,
                            y,
                            ((x as f64 / 512.0) * 255.0) as u16,
                            ((y as f64 / 512.0) * 255.0) as u16,
                            if y % 2 == 0 { 0x00 } else { 0xff },
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
