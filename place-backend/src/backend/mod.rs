use std::{
    net::Ipv6Addr,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
};

use tokio::{sync::broadcast, task::JoinHandle};

use crate::{
    place::SharedImageHandle,
    settings::{BackendType, Settings},
    utils::Color,
    PResult,
};

#[cfg(feature = "backend-smoltcp")]
mod smoltcp;

#[cfg(not(all(feature = "backend-smoltcp")))]
compile_error!(
    "No backends enabled. Please enable at least one backend with the `backend-*` features."
);

pub struct PixelRequest {
    pub pos: (u16, u16),
    pub color: Color,
    pub size: u8,
}

impl PixelRequest {
    /// Parses an IP address in form of 2602:fa9b:42:SXXX:YYY:RR:GG:BB into a PixelRequest.
    #[inline]
    pub const fn from_ipv6(ip: &Ipv6Addr) -> Self {
        let octets = ip.segments();

        let size = ((octets[3] & 0x3000) >> 12) as u8;
        // clamp size to 1 or 2 (without branching)
        let size = (size & 2) | 1;

        let x = octets[3] & 0xfff;
        let y = octets[4] & 0xfff;

        let r = (octets[5] & 0xff) as u8;
        let g = (octets[6] & 0xff) as u8;
        let b = (octets[7] & 0xff) as u8;

        Self {
            pos: (x, y),
            color: Color::rgb(r, g, b),
            size,
        }
    }
}

pub struct PacketCounter {
    pps: AtomicU32,
    counter: AtomicU32,
}

impl PacketCounter {
    #[inline]
    pub fn increment(&self) {
        self.counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    fn reset_pps(&self) -> u32 {
        let pps = self.counter.swap(0, Ordering::Relaxed);
        self.pps.store(pps, Ordering::Relaxed);
        pps
    }

    pub async fn pps_counter_task(self: Arc<Self>, pps_sender: broadcast::Sender<u32>) -> PResult<()> {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            let pps = self.reset_pps();
            pps_sender.send(pps)?;
        }
    }
}

pub trait NetworkBackend: Send + Sync {
    fn start(self: Box<Self>) -> JoinHandle<PResult<()>>;
}

pub fn backend_factory(
    settings: &Settings,
    image: SharedImageHandle,
) -> PResult<Box<dyn NetworkBackend>> {
    match settings.backend.backend_type {
        #[cfg(feature = "backend-smoltcp")]
        BackendType::Smoltcp => smoltcp::SmoltcpNetworkBackend::new(&settings, image),

        #[allow(unreachable_patterns)]
        _ => Err(format!(
            "Specified backend '{:?}' has not been compiled in.",
            settings.backend.backend_type
        )
        .into()),
    }
}
