use std::sync::Arc;

use crate::{place::SharedImageHandle, settings::Settings, PResult};

use super::{NetworkBackend, PacketCounter};

pub struct TunNetworkBackend {}

impl TunNetworkBackend {
    pub fn new(
        settings: &Settings,
        image: SharedImageHandle,
        packet_counter: Arc<PacketCounter>,
    ) -> PResult<Box<dyn NetworkBackend>> {
        

        Ok(Box::new(Self {}))
    }
}

impl NetworkBackend for TunNetworkBackend {
    fn start(self: Box<Self>) -> tokio::task::JoinHandle<PResult<()>> {
        todo!()
    }
}