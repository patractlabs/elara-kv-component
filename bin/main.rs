use std::{sync::Arc, time::Duration};

use anyhow::Result;
use futures::StreamExt;
use tokio::sync::RwLock;
use tokio_tungstenite::tungstenite::{Error, Message};

use elara_kv_component::message::WsClientError;
use elara_kv_component::substrate::dispatch::{
    ChainAllHeadDispatcher, ChainFinalizedHeadDispatcher, ChainNewHeadDispatcher,
    DispatcherHandler, GrandpaJustificationDispatcher, StateRuntimeVersionDispatcher,
    StateStorageDispatcher,
};
use elara_kv_component::substrate::request_handler::RequestHandler;
use elara_kv_component::{
    cmd::*,
    rpc_client::{self, ArcRpcClient, RpcClient, RpcClients},
    websocket::{WsConnection, WsConnections, WsServer},
};

const CONN_EXPIRED_TIME_SECS: u64 = 10;
const CHECK_CONN_ALIVE_SECS: u64 = 10;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let opt = CliOpts::init();

    let config = opt.parse()?;
    log::debug!("Load config: {:#?}", config);

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
    cfg: &ServiceConfig,
    connections: WsConnections,
) -> rpc_client::Result<RpcClients> {
    let mut clients: RpcClients = Default::default();
    // started to subscribe chain node by ws client
    for (chain, cfg) in cfg.nodes.iter() {
        let client = RpcClient::new(chain.clone(), cfg.url.clone()).await?;
        subscribe_chain(connections.clone(), &client).await;
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
                Ok(()) => register_subscriptions(&*client, connections.clone()).await,
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

async fn register_subscriptions(
    client: &RpcClient,
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

    handler.start_dispatch(client, conns).await
}

async fn subscribe_chain(connections: WsConnections, client: &RpcClient) {
    log::info!(
        "Start to subscribe chain `{}` from `{}`",
        client.chain(),
        client.addr()
    );
    register_subscriptions(client, connections)
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
        log::debug!("recv a message: {:?}", msg);
        match msg {
            Ok(msg) => {
                if msg.is_empty() {
                    continue;
                }
                match msg {
                    Message::Pong(_) => {}

                    Message::Binary(_) => {}

                    Message::Close(_) => {
                        let _res = connection.send_close().await;
                        break;
                    }

                    Message::Ping(_) => {
                        let _res = connection
                            .send_message(Message::Pong(b"pong".to_vec()))
                            .await;
                    }

                    Message::Text(s) => {
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

            // closed connection
            Err(Error::ConnectionClosed) => break,

            Err(err) => {
                log::warn!("Err occurred when receive a message: {}", err);
                break;
            }
        }
    }

    connection.close();
    log::info!("Closed connection to peer {}", connection.addr());
}
