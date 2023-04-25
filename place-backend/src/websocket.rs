use std::sync::Arc;

use crate::{place::SharedImageHandle, settings::Settings, PResult};
use futures::{stream::StreamExt, SinkExt};
use hyper::{Body, Request, Response};
use hyper_tungstenite::{tungstenite::Message, HyperWebsocket};
use image::ImageEncoder;
use image::{codecs::png, ColorType};
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
        image_handle: SharedImageHandle,
    ) -> PResult<Response<Body>> {
        if hyper_tungstenite::is_upgrade_request(&request) {
            if request.uri().path() == "/ws" {
                let (response, websocket) = hyper_tungstenite::upgrade(&mut request, None)?;

                // Spawn a task to handle the websocket connection.
                tokio::spawn(async move {
                    if let Err(e) = WebSocketServer::serve_websocket(websocket, image_handle).await
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
        image_handle: SharedImageHandle,
    ) -> PResult<()> {
        let websocket = websocket.await?;
        let (mut sender, mut receiver) = websocket.split();

        let img_future = tokio::spawn(async move {
            loop {
                let data = {
                    let image = image_handle.get_image().await;
                    let mut writer = Vec::new();
                    let encoder = png::PngEncoder::new(&mut writer);
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
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
        });

        while let Some(message) = receiver.next().await {
            match message? {
                _ => {}
            }
        }

        tokio::join!(img_future);

        Ok(())
    }

    async fn run(&mut self, image_handle: SharedImageHandle) -> PResult<()> {
        // The config doesn't change during lifetime of the server, so we can serialize it and turn it
        // into &'static str to avoid making redundant copies of the string on every request.
        let serialized_config: &'static str =
            Box::leak(serde_json::to_string(&self.config_info)?.into_boxed_str());

        loop {
            let (stream, addr) = self.socket.accept().await?;
            log::info!("New connection from {}", addr);

            let image_handle = image_handle.clone();
            let connection = self
                .http
                .serve_connection(
                    stream,
                    hyper::service::service_fn(move |request| {
                        WebSocketServer::handle_request(
                            request,
                            serialized_config,
                            image_handle.clone(),
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

    pub fn start_server(mut self, image_handle: SharedImageHandle) -> JoinHandle<PResult<()>> {
        tokio::spawn(async move { self.run(image_handle).await })
    }
}
