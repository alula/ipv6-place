use std::time::Duration;

use crate::SharedContext;
use crate::{settings::Settings, PResult};
use futures::{stream::StreamExt, SinkExt};
use hyper::{Body, Request, Response};
use hyper_tungstenite::{tungstenite::Message, HyperWebsocket};
use image::{codecs::png, ColorType};
use image::{ImageBuffer, ImageEncoder, Rgba};
use serde::{Deserialize, Serialize};
use tokio::{net::TcpListener, task::JoinHandle};

pub struct WebSocketServer {
    socket: TcpListener,
    http: hyper::server::conn::Http,
    config_info: ServerConfigInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ServerConfigInfo {
    ipv6_prefix: String,
    canvas_size: u16,
}

impl WebSocketServer {
    pub async fn new(settings: &Settings) -> PResult<WebSocketServer> {
        let socket = TcpListener::bind(&settings.websocket.listen_addr).await?;
        log::info!(
            "HTTP/WebSocket listening on on http://{}",
            socket.local_addr()?
        );

        let mut http = hyper::server::conn::Http::new();
        http.http1_only(true);
        http.http1_keep_alive(true);

        let config_info = {
            let prefix48 = settings.backend.prefix48.segments();
            ServerConfigInfo {
                ipv6_prefix: format!(
                    "{:x}:{:x}:{:x}::SXXX:YYY:RR:GG:BB",
                    prefix48[0], prefix48[1], prefix48[2]
                ),
                canvas_size: settings.canvas.size.get(),
            }
        };

        Ok(WebSocketServer {
            socket,
            http,
            config_info,
        })
    }

    async fn handle_request(
        mut request: Request<Body>,
        serialized_config: &'static str,
        shared_context: SharedContext,
    ) -> PResult<Response<Body>> {
        if hyper_tungstenite::is_upgrade_request(&request) {
            if request.uri().path() == "/ws" {
                let (response, websocket) = hyper_tungstenite::upgrade(&mut request, None)?;

                // Spawn a task to handle the websocket connection.
                tokio::spawn(async move {
                    if let Err(e) =
                        WebSocketServer::serve_websocket(websocket, shared_context).await
                    {
                        log::error!("Error in websocket connection: {}", e);
                    }
                });

                // Return the response so the spawned future can continue.
                return Ok(response);
            }
        } else if request.uri().path() == "/config.json" {
            let response = Response::builder()
                .status(200)
                .header("Content-Type", "application/json")
                .body(Body::from(serialized_config))?;
            return Ok(response);
        }

        let response = Response::builder()
            .status(404)
            .body(Body::from("Not Found"))?;
        return Ok(response);
    }

    async fn serve_websocket(
        websocket: HyperWebsocket,
        mut shared_context: SharedContext,
    ) -> PResult<()> {
        let websocket = websocket.await?;
        let (mut sender, mut receiver) = websocket.split();

        let sender_future = tokio::spawn(async move {
            let mut image = {
                let (width, height) = shared_context.image.get_dimensions();
                ImageBuffer::<Rgba<u8>, Vec<u8>>::new(width, height)
            };

            let frame_interval = std::time::Duration::from_millis(1000) / 15;

            loop {
                let start = std::time::Instant::now();
                if let Ok(pps) = shared_context.pps_receiver.try_recv() {
                    if sender
                        .feed(Message::Text(format!("{{\"evt\":{}}}", pps)))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }

                let data = {
                    {
                        let shared_image = unsafe { shared_context.image.get_image() };
                        image.copy_from_slice(shared_image.as_raw().as_slice());
                    }

                    let mut writer = Vec::new();
                    let encoder = png::PngEncoder::new_with_quality(
                        &mut writer,
                        png::CompressionType::Fast,
                        png::FilterType::Adaptive,
                    );
                    if encoder
                        .write_image(
                            image.as_raw(),
                            image.width(),
                            image.height(),
                            ColorType::Rgba8,
                        )
                        .is_err()
                    {
                        continue;
                    }

                    writer
                };

                if sender.send(Message::Binary(data)).await.is_err() {
                    break;
                }

                let now = std::time::Instant::now();
                let elapsed = now - start;

                log::debug!("Elapsed = {:?}, interval = {:?}", elapsed, frame_interval);

                if elapsed < frame_interval {
                    tokio::time::sleep(frame_interval - elapsed).await;
                } else {
                    // give some time to calm down in case we're starting to get laggy
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
//                tokio::task::yield_now().await;
            }
        });

        while let Some(message) = receiver.next().await {
            match message? {
                Message::Close(_) => break,
                _ => {}
            }
        }

        sender_future.abort();

        Ok(())
    }

    async fn run(&mut self, shared_context: SharedContext) -> PResult<()> {
        // The config doesn't change during lifetime of the server, so we can serialize it and turn it
        // into &'static str to avoid making redundant copies of the string on every request.
        let serialized_config: &'static str =
            Box::leak(serde_json::to_string(&self.config_info)?.into_boxed_str());

        loop {
            let (stream, addr) = self.socket.accept().await?;
            log::info!("New connection from {}", addr);

            let shared_context = shared_context.clone();
            let connection = self
                .http
                .serve_connection(
                    stream,
                    hyper::service::service_fn(move |request| {
                        WebSocketServer::handle_request(
                            request,
                            serialized_config,
                            shared_context.clone(),
                        )
                    }),
                )
                .with_upgrades();

            tokio::spawn(async move {
                if let Err(err) = connection.await {
                    println!("Error serving HTTP connection: {:?}", err);
                }
            });
        }
    }

    pub fn start_server(mut self, shared_context: SharedContext) -> JoinHandle<PResult<()>> {
        tokio::spawn(async move { self.run(shared_context).await })
    }
}
