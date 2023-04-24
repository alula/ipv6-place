use std::sync::Arc;

use tokio::task::JoinSet;

mod backend;
mod place;
mod settings;
mod utils;
mod websocket;

pub type PResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;

#[derive(Clone)]
pub struct SharedContext {
    pub image: place::SharedImageHandle,
    pub packet_counter: Arc<backend::PacketCounter>,
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
    let backend = backend::backend_factory(&settings, place.image.clone())?;

    let mut join_set = JoinSet::new();

    let image_handle = place.image.clone();
    join_set.spawn(async move { websocket.start_server(image_handle).await? });
    join_set.spawn(async move { place.start_diffing_task().await? });
    join_set.spawn(async move { backend.start().await? });

    while let Some(result) = join_set.join_next().await {
        result??;
    }

    Ok(())
}
