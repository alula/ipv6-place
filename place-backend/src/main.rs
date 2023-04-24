use tokio::task::JoinSet;

mod backend;
mod place;
mod settings;
mod utils;
mod websocket;

pub type PResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;

#[tokio::main]
async fn main() -> PResult<()> {
    let log_level = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
    pretty_env_logger::formatted_timed_builder()
        .filter_level(log_level.parse()?)
        .try_init()?;

    let settings = settings::Settings::new()?;
    log::info!("settings = {:?}", settings);

    let mut place = place::Place::new(&settings.canvas)?;
    let mut websocket = websocket::WebSocketServer::new(&settings).await?;

    let mut join_set = JoinSet::new();

    join_set.spawn(async move { websocket.start_server().await? });
    join_set.spawn(async move { place.start_diffing_task().await? });

    while let Some(result) = join_set.join_next().await {
        result??;
    }

    Ok(())
}
