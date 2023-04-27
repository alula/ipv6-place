use image::{ImageBuffer, ImageFormat, Rgba, RgbaImage};
use std::{cell::UnsafeCell, fs::File, io::BufReader, path::PathBuf, sync::Arc};
use tokio::{sync::broadcast, task::JoinHandle};

use crate::{settings::CanvasSettings, utils::Color, PResult};

/// (UN)SAFETY NOTE:
/// We avoid locking here to get a 10-25% performance boost.
///
/// Data consistency doesn't really matter in our case, but use of this buffer
/// must be taken with care or we can easily shoot ourselves in the foot.
/// Eg. this introduced an issue in optimized builds that caused PNGs sent via websocket
/// to be corrupted, because the image changed while it was being encoded on another thread.
/// This has been easily worked around by making a copy of the image before encoding it.
pub struct SharedImageHandle {
    data: Arc<UnsafeCell<RgbaImage>>,
}

impl SharedImageHandle {
    pub fn new(data: RgbaImage) -> SharedImageHandle {
        SharedImageHandle {
            // data: Arc::new(RwLock::new(data)),
            data: Arc::new(UnsafeCell::new(data)),
        }
    }

    pub fn put(&self, x: u32, y: u32, color: Color, big: bool) {
        // let mut image = self.data.write().await;
        let image = unsafe { &mut *self.data.get() };
        if x >= image.dimensions().0 || y >= image.dimensions().1 {
            return;
        }

        if let Some(i) = image.get_pixel_mut_checked(x, y) {
            *i = color.into_rgba()
        };
        if big {
            if let Some(i) = image.get_pixel_mut_checked(x + 1, y) {
                *i = color.into_rgba()
            };
            if let Some(i) = image.get_pixel_mut_checked(x, y + 1) {
                *i = color.into_rgba()
            };
            if let Some(i) = image.get_pixel_mut_checked(x + 1, y + 1) {
                *i = color.into_rgba()
            };
        }
    }

    pub fn get_dimensions(&self) -> (u32, u32) {
        let image = unsafe { &mut *self.data.get() };
        image.dimensions()
    }

    /// SAFETY: See comment in SharedImageHandle for details.
    pub unsafe fn get_image(&self) -> &RgbaImage {
        let image = unsafe { &mut *self.data.get() };
        image
    }
}

/// SAFETY: See comment in SharedImageHandle for details.
unsafe impl Send for SharedImageHandle {}
/// SAFETY: See comment in SharedImageHandle for details.
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

        let mut image = {
            let (width, height) = self.image.get_dimensions();
            ImageBuffer::<Rgba<u8>, Vec<u8>>::new(width, height)
        };

        let shared_image = unsafe { self.image.get_image() };
        image.copy_from_slice(shared_image.as_raw().as_slice());

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
        let (x, y) = place.image.get_dimensions();
        for x in 0..x {
            for y in 0..y {
                place.image.put(
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
