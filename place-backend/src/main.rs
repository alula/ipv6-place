use std::sync::Arc;

use tokio::{sync::broadcast, task::JoinSet};

mod backend;
mod place;
mod settings;
mod utils;
mod websocket;

pub type PResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;

pub struct SharedContext {
    pub image: place::SharedImageHandle,
    pub pps_receiver: broadcast::Receiver<u32>,
}

impl Clone for SharedContext {
    fn clone(&self) -> Self {
        Self {
            image: self.image.clone(),
            pps_receiver: self.pps_receiver.resubscribe(),
        }
    }
}

#[tokio::main]
async fn main() -> PResult<()> {
    let log_level = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
    pretty_env_logger::formatted_timed_builder()
        .filter_level(log_level.parse()?)
        .try_init()?;

    let settings = settings::Settings::new()?;
    log::info!("settings = {:?}", settings);

    let place = place::Place::new(&settings.canvas)?;
    let websocket = websocket::WebSocketServer::new(&settings).await?;
    let packet_counter = backend::PacketCounter::new();
    let backend = backend::backend_factory(&settings, place.image.clone(), packet_counter.clone())?;
    let (pps_sender, pps_receiver) = broadcast::channel::<u32>(1);

    let mut join_set = JoinSet::new();

    let shared_context = SharedContext {
        image: place.image.clone(),
        pps_receiver,
    };
    join_set.spawn(async move { packet_counter.start_pps_counter(pps_sender).await? });
    join_set.spawn(async move { websocket.start_server(shared_context).await? });
    join_set.spawn(async move { place.start_diffing_task().await? });
    join_set.spawn(async move { backend.start().await? });

    while let Some(result) = join_set.join_next().await {
        result??;
    }

    Ok(())
}
