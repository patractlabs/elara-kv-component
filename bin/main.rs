mod cmd;

use elara_kv_component::config::*;
use elara_kv_component::error::Result;
use elara_kv_component::rpc_client::RpcClient;
use futures::StreamExt;
use log::*;
use std::sync::Arc;
use tokio_tungstenite::tungstenite;
use tungstenite::{Error, Message};

use elara_kv_component::rpc_client;
use elara_kv_component::websocket::{WsConnection, WsConnections, WsServer};
use elara_kv_component::{kusama, polkadot};
use std::collections::HashMap;
use tokio::sync::Mutex;
use tokio::time::Duration;

use structopt::StructOpt;

type WsClients = HashMap<String, Arc<Mutex<RpcClient>>>;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let opt = cmd::Opt::from_args();

    info!("Load config from: {}", &opt.config);
    let cfg = load_config(&opt.config).expect("Illegal config path");
    let cfg: Config = toml::from_str(&*cfg).expect("Config is illegal");
    let cfg = cfg.validate().expect("Config is illegal");

    debug!("Load config: {:#?}", &cfg);

    let addr = cfg.ws.addr.as_str();
    let server = WsServer::bind(addr)
        .await
        .expect(&format!("Cannot listen {}", addr));
    info!("Started WebSocket server at {}", addr);

    // started to subscribe chain node by ws client
    let mut connections = WsConnections::new();
    tokio::spawn(remove_expired_connections(connections.clone()));

    // TODO: impl the logic of re-connection
    let _clients = create_clients(&cfg, connections.clone())
        .await
        .expect("Cannot subscribe node");

    // accept a new connection
    loop {
        match server.accept(cfg.clone()).await {
            Ok(conn) => {
                // TODO: add config for chain
                connections.add(conn.clone()).await;

                // We register the configured node handlers here
                register_handlers(conn.clone()).await;
                info!("New WebSocket connection: {}", conn.addr());
                info!("Total connection num: {}", connections.len().await);

                tokio::spawn(handle_connection(conn));
            }
            Err(err) => {
                warn!("Error occurred when accept a new connection: {}", err);
            }
        };
    }
}

const CONN_EXPIRED_TIME_SECS: u64 = 10;

// we remove unlived connection every 5s
async fn remove_expired_connections(mut conns: WsConnections) {
    loop {
        let mut expired = vec![];
        for (addr, conn) in conns.inner().read().await.iter() {
            if conn.closed() {
                expired.push(*addr);
            }
        }

        for addr in expired {
            info!("Removed a expired connection: {}", addr);
            conns.remove(&addr).await;
            info!("Total connection num: {}", conns.len().await);
        }

        tokio::time::sleep(Duration::from_secs(CONN_EXPIRED_TIME_SECS)).await;
    }
}

async fn create_clients(
    cfg: &Config,
    connections: WsConnections,
) -> rpc_client::Result<WsClients> {
    let mut clients: WsClients = Default::default();
    // started to subscribe chain node by ws client
    for (node, cfg) in cfg.nodes.iter() {
        let client = RpcClient::new(node.clone(), cfg.addr.clone())
            .await
            .unwrap_or_else(|_| panic!("Cannot connect to {}: {}", node, cfg.addr));

        match node.as_str() {
            polkadot::NODE_NAME => subscribe_polkadot(connections.clone(), &client).await,
            kusama::NODE_NAME => subscribe_kusama(connections.clone(), &client).await,

            // TODO:
            _ => unimplemented!(),
        };

        // maintains these clients
        clients.insert(node.to_string(), Arc::new(Mutex::new(client)));
    }

    Ok(clients)
}

async fn subscribe_polkadot(connections: WsConnections, client: &RpcClient) {
    info!("Start to subscribe polkadot from `{}`", client.addr());
    let stream = polkadot::rpc_client::start_subscribe(client)
        .await
        .expect("Cannot subscribe polkadot node");
    stream.start(connections.clone());
}

async fn subscribe_kusama(connections: WsConnections, client: &RpcClient) {
    info!("Start to subscribe kusama from `{}`", client.addr());
    let stream = kusama::rpc_client::start_subscribe(client)
        .await
        .expect("Cannot subscribe polkadot node");
    stream.start(connections.clone());
}

async fn register_handlers(mut conn: WsConnection) {
    conn.register_message_handler(
        polkadot::NODE_NAME,
        polkadot::client::RequestHandler::new(conn.clone()),
    )
    .await;

    conn.register_message_handler(
        kusama::NODE_NAME,
        kusama::client::RequestHandler::new(conn.clone()),
    )
    .await;
}

async fn handle_connection(connection: WsConnection) {
    let receiver = connection.receiver();
    let mut receiver = receiver.lock().await;

    while let Some(msg) = receiver.next().await {
        debug!("recv a message: {:?}", msg);
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
                        // TODO: need we to handle this?
                        let _res = connection
                            .send_message(Message::Pong(b"pong".to_vec()))
                            .await;
                    }

                    Message::Text(s) => {
                        let res = connection.handle_message(s).await;
                        match res {
                            Ok(()) => {}
                            Err(err) => {
                                warn!(
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
                warn!("Err occurred when receive a message: {}", err);
                break;
            }
        }
    }

    info!("Closed connection to peer {}", connection.addr());
}
