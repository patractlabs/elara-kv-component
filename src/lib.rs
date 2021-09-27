pub mod cmd;
pub mod compression;
pub mod config;
pub mod message;
mod misc;
pub mod rpc_client;
pub mod session;
pub mod substrate;
pub mod websocket;

use anyhow::Result;
use futures::StreamExt;
pub use misc::*;
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};
use tokio::sync::RwLock;
use tokio_tungstenite::tungstenite::{Error, Message};

use crate::config::ServiceConfig;
use crate::message::WsClientError;
use crate::rpc_client::RpcClientCtx;
use crate::substrate::dispatch::{
    ChainAllHeadDispatcher, ChainFinalizedHeadDispatcher, ChainNewHeadDispatcher,
    DispatcherHandler, GrandpaJustificationDispatcher, StateRuntimeVersionDispatcher,
    StateStorageDispatcher,
};
use crate::substrate::request_handler::RequestHandler;
use crate::{
    rpc_client::{ArcRpcClient, RpcClient, RpcClients},
    websocket::{WsConnection, WsConnections, WsServer},
};

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, Default)]
#[serde(transparent)]
#[repr(transparent)]
pub struct Chain(String);

impl std::fmt::Display for Chain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<String> for Chain {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for Chain {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

const CONN_EXPIRED_TIME_SECS: u64 = 10;
const CHECK_CONN_ALIVE_SECS: u64 = 10;

pub async fn start_server(config: ServiceConfig) -> Result<()> {
    let server = WsServer::bind(config.ws.addr.as_str()).await?;
    log::info!("Started WebSocket server at {}", config.ws.addr);

    // started to subscribe chain node by ws client
    let mut connections = WsConnections::new();
    tokio::spawn(remove_expired_connections(connections.clone()));

    let clients = create_clients(&config, connections.clone()).await?;

    // accept a new connection
    loop {
        match server.accept(clients.clone(), config.clone()).await {
            Ok(conn) => {
                connections.add(conn.clone()).await;

                // We register the configured node handlers here
                register_handlers(conn.clone()).await;
                log::info!("New WebSocket connection: {}", conn.addr());
                log::info!("Total connection num: {}", connections.len().await);

                tokio::spawn(handle_connection(conn));
            }
            Err(err) => {
                log::warn!("Error occurred when accept a new connection: {}", err);
            }
        };
    }
}

// we remove unlived connection every 5s
async fn remove_expired_connections(mut conns: WsConnections) {
    loop {
        let mut expired = vec![];
        for (addr, conn) in conns.inner().read().await.iter() {
            log::debug!("{}", conn);
            if conn.is_closed() {
                expired.push(*addr);
            }
        }

        for addr in expired {
            log::info!("Removed a expired connection: {}", addr);
            conns.remove(&addr).await;
            log::info!("Total connection num: {}", conns.len().await);
        }

        tokio::time::sleep(Duration::from_secs(CONN_EXPIRED_TIME_SECS)).await;
    }
}

async fn create_clients(
    config: &ServiceConfig,
    connections: WsConnections,
) -> rpc_client::Result<RpcClients> {
    let mut clients: RpcClients = Default::default();
    // started to subscribe chain node by ws client
    for (chain, node) in config.nodes.iter() {
        let mut client = RpcClient::new(chain.clone(), node.url.clone(), config.client).await?;
        subscribe_chain(connections.clone(), &mut client).await;
        let client = Arc::new(RwLock::new(client));
        tokio::spawn(health_check(connections.clone(), client.clone()));
        // maintains these clients
        clients.insert(chain.clone(), client);
    }

    Ok(clients)
}

// if rpc client is not alive, we reconnect to peer
async fn health_check(connections: WsConnections, client: ArcRpcClient) {
    loop {
        tokio::time::sleep(Duration::from_secs(CHECK_CONN_ALIVE_SECS)).await;

        let mut client = client.write().await;
        if !client.is_alive().await {
            log::info!(
                "Trying to reconnect to `{}` to `{}`...",
                client.chain(),
                client.addr()
            );
            let res = client.reconnect().await;

            let res = match res {
                Err(err) => Err(err),
                Ok(()) => start_subscriptions(&mut client, connections.clone()).await,
            };

            if let Err(err) = res {
                log::warn!(
                    "Error occurred when reconnect to `{}: {}`: {:?}",
                    client.chain(),
                    client.addr(),
                    err
                );
            }
        }
    }
}

/// register all dispatchers and init client context.
async fn start_subscriptions(
    client: &mut RpcClient,
    conns: WsConnections,
) -> Result<(), WsClientError> {
    let chain = client.chain();
    let mut handler = DispatcherHandler::new();
    handler.register_dispatcher(StateStorageDispatcher::new(chain.clone()));
    handler.register_dispatcher(StateRuntimeVersionDispatcher::new(chain.clone()));
    handler.register_dispatcher(ChainNewHeadDispatcher::new(chain.clone()));
    handler.register_dispatcher(ChainFinalizedHeadDispatcher::new(chain.clone()));
    handler.register_dispatcher(ChainAllHeadDispatcher::new(chain.clone()));
    handler.register_dispatcher(GrandpaJustificationDispatcher::new(chain));
    handler.start_dispatch(client, conns).await?;
    let _ = client.ctx.insert(RpcClientCtx { handler });
    Ok(())
}

async fn subscribe_chain(connections: WsConnections, client: &mut RpcClient) {
    log::info!(
        "Start to subscribe chain `{}` from `{}`",
        client.chain(),
        client.addr()
    );
    start_subscriptions(client, connections)
        .await
        .unwrap_or_else(|_| panic!("Cannot subscribe chain `{}`", client.chain()))
}

async fn register_handlers(mut conn: WsConnection) {
    for (chain, _) in conn.config().nodes.clone() {
        let handler = RequestHandler::new(&chain);
        conn.register_message_handler(chain, handler).await;
    }
}

async fn handle_connection(connection: WsConnection) {
    let receiver = connection.receiver();
    let mut receiver = receiver.lock().await;

    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(msg) => {
                if msg.is_empty() {
                    continue;
                }
                match msg {
                    Message::Pong(_) => {}

                    Message::Binary(_) => {}

                    Message::Close(_) => {
                        let res = connection.send_close().await;
                        handle_result(res, "handle close message");
                        break;
                    }

                    Message::Ping(_) => {
                        let res = connection
                            .send_message(Message::Pong(b"pong".to_vec()))
                            .await;
                        handle_result(res, "handle ping message");
                    }

                    Message::Text(s) => {
                        log::debug!(
                            "Handle a request for connection {}: {}",
                            connection.addr(),
                            s
                        );
                        let res = connection.handle_message(s).await;
                        match res {
                            Ok(()) => {}
                            Err(err) => {
                                log::warn!(
                                    "Error occurred when send response to peer `{}`: {}",
                                    connection.addr(),
                                    err
                                );
                            }
                        };
                    }
                };
            }

            Err(Error::ConnectionClosed) => break,

            Err(err) => {
                log::warn!("Error occurred when receive a message: {}", err);
                break;
            }
        }
    }

    connection.close().await;
    log::info!("Closed connection to peer {}", connection.addr());
}
