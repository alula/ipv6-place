use std::sync::Arc;

use crate::{settings::Settings, PResult};
use futures::stream::StreamExt;
use hyper::{Body, Request, Response};
use hyper_tungstenite::HyperWebsocket;
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
    ) -> PResult<Response<Body>> {
        if hyper_tungstenite::is_upgrade_request(&request) {
            if request.uri().path() == "/" {
                let (response, websocket) = hyper_tungstenite::upgrade(&mut request, None)?;

                // Spawn a task to handle the websocket connection.
                tokio::spawn(async move {
                    if let Err(e) = WebSocketServer::serve_websocket(websocket).await {
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

    async fn serve_websocket(websocket: HyperWebsocket) -> PResult<()> {
        let mut websocket = websocket.await?;
        let (mut sender, mut receiver) = websocket.split();
        while let Some(message) = receiver.next().await {
            match message? {
                _ => {}
            }
        }

        Ok(())
    }

    async fn run(&mut self) -> PResult<()> {
        let serialized_config: &'static str = Box::leak(serde_json::to_string(&self.config_info)?.into_boxed_str());

        loop {
            let (stream, addr) = self.socket.accept().await?;
            log::info!("New connection from {}", addr);

            let connection = self
                .http
                .serve_connection(
                    stream,
                    hyper::service::service_fn(move |request| {
                        WebSocketServer::handle_request(request, serialized_config)
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

    pub fn start_server(mut self) -> JoinHandle<PResult<()>> {
        tokio::spawn(async move { self.run().await })
    }
}
