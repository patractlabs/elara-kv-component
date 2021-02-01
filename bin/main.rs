mod cmd;

use elara_kv_component::config::*;
use elara_kv_component::error::Result;
use elara_kv_component::rpc_client::RpcClient;
use futures::StreamExt;
use log::*;
use std::sync::Arc;
use tokio_tungstenite::tungstenite;
use tungstenite::{Error, Message};

use elara_kv_component::polkadot;
use elara_kv_component::rpc_client;
use elara_kv_component::websocket::{WsConnection, WsConnections, WsServer};
use std::collections::HashMap;
use tokio::sync::Mutex;
use tokio::time::Duration;

use elara_kv_component::error::ServiceError::WsServerError;
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
        .expect(&*format!("Cannot listen {}", addr));
    info!("Started WebSocket server at {}", addr);

    // started to subscribe chain node by ws client
    let mut connections = WsConnections::new();
    tokio::spawn(remove_expired_connections(connections.clone()));

    // TODO: impl the logic of re-connection
    let clients = create_clients(&cfg, connections.clone())
        .await
        .expect("Cannot subscribe node");

    // accept a new connection
    loop {
        match server.accept(cfg.clone()).await {
            Ok(connection) => {
                connections.add(connection.clone()).await;

                info!("New WebSocket connection: {}", connection.addr());
                info!("Total connection num: {}", connections.len().await);

                tokio::spawn(handle_connection(connection));
            }
            Err(err) => {
                warn!("Error occurred when accept a new connection: {}", err);
            }
        };
    }
}

// we remove unlived connection every 5s
async fn remove_expired_connections(mut conns: WsConnections) {
    loop {
        let mut expired = vec![];
        for (addr, conn) in conns.inner().read().await.iter() {
            if conn.closed() {
                expired.push(addr.clone());
            }
        }

        for addr in expired {
            conns.remove(&addr).await;
            info!("Removed a expired connection: {}", addr);
            info!("Total connection num: {}", conns.len().await);
        }

        tokio::time::sleep(Duration::from_secs(5)).await;
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
            .expect(&format!("Cannot connect to {}: {}", node, cfg.addr));

        match node.as_str() {
            polkadot::consts::NODE_NAME => {
                let polkadot_stream =
                    polkadot::rpc_client::start_polkadot_subscribe(&client).await?;
                polkadot_stream.start(connections.clone())
            }

            _ => unimplemented!(),
        }
        clients.insert(node.to_string(), Arc::new(Mutex::new(client)));
    }

    Ok(clients)
}

async fn handle_connection(connection: WsConnection) {
    let receiver = connection.receiver();
    let mut receiver = receiver.lock().await;
    loop {
        let msg = receiver.next().await;
        match msg {
            Some(Ok(msg)) => {
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
            Some(Err(Error::ConnectionClosed)) | None => break,

            Some(Err(err)) => {
                warn!("Err {}", err);
            }
        }
    }

    match connection.close().await {
        Ok(()) => {}
        Err(WsServerError(Error::ConnectionClosed)) => {}
        Err(WsServerError(err)) => {
            warn!(
                "Error occurred when closed connection to {}: {}",
                connection.addr(),
                err,
            );
        }
        _ => {}
    };

    info!("Closed connection to peer {}", connection.addr());
}
