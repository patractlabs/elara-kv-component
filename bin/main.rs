#[macro_use]
use lazy_static::lazy_static;

use elara_kv_component::config::*;
use elara_kv_component::error::Result;
use elara_kv_component::kafka::{KVSubscriber, KvConsumer, LogLevel, OwnedMessage};
use elara_kv_component::message::ResponseErrorMessage;
use elara_kv_component::websocket::{collect_subscribed_storage, WsConnection, WsServer};

use elara_kv_component::client::WsClient;
use futures::{SinkExt, StreamExt};
use log::*;
use rdkafka::Message as KafkaMessage;
use std::sync::Arc;
use tokio::sync::broadcast::Receiver;
use tokio_tungstenite::tungstenite;
use tungstenite::{Error, Message};

use jsonrpc_core::{Output, Response, Success};
use std::collections::HashMap;
use tokio::sync::Mutex;

// lazy_static! {
//     static ref HASHMAP: HashMap<&'static String, > = {
//         let mut m = HashMap::new();
//         m.insert(0, "foo");
//         m.insert(1, "bar");
//         m.insert(2, "baz");
//         m
//     };
// }

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    // TODO; refine config
    // TODO: add cli
    let path = "bin/config.toml";
    info!("Load config from: {}", path);
    let cfg = load_config(path).expect("Illegal config path");
    let cfg: Config = toml::from_str(&*cfg).expect("Config is illegal");
    let cfg = cfg.validate().expect("Config is illegal");

    debug!("Load config: {:#?}", &cfg);
    let consumer = KvConsumer::new(cfg.kafka.config.into_iter(), LogLevel::Debug);

    let topics: Vec<&str> = cfg.kafka.topics.iter().map(|s| s.as_str()).collect();
    info!("Subscribing kafka topic: {:?}", &topics);
    consumer.subscribe(&*topics)?;

    let subscriber = Arc::new(KVSubscriber::new(Arc::new(consumer), 100));
    // start subscribing some topics
    subscriber.start();

    let addr = cfg.ws.addr.as_str();
    let server = WsServer::bind(addr)
        .await
        .expect(&*format!("Cannot listen {}", addr));
    info!("Started ws server at {}", addr);

    let mut clients: WsClients = Default::default();
    // started to subscribe chain node by ws client
    for (node, cfg) in cfg.nodes.iter() {
        let mut client = WsClient::connect(cfg.addr.clone())
            .await
            .expect(&format!("Cannot connect to {:?}", node));

        let mut receiver = client.receiver();

        use tokio::sync::broadcast::error::RecvError;
        tokio::spawn(async move {
            loop {
                let resp = receiver.recv().await;

                match resp {
                    Err(RecvError::Closed) => return,
                    Err(RecvError::Lagged(_)) => {}
                    Ok(resp) => {
                        info!("{:?}", resp);
                    }
                }
            }
        });

        clients.insert(node.to_string(), Arc::new(Mutex::new(client)));
    }

    // accept a new connection
    loop {
        match server.accept().await {
            Ok(connection) => {
                handle_connection(connection, subscriber.clone());
            }
            Err(err) => {
                warn!("Error occurred when accept a new connection: {}", err);
            }
        };
    }
}

type WsClients = HashMap<String, Arc<Mutex<WsClient>>>;

fn handle_connection(
    connection: WsConnection,
    subscriber: Arc<KVSubscriber>,
    clients: WsClients,
) {
    info!("New WebSocket connection: {}", connection.addr);
    tokio::spawn(handle_request(connection.clone(), subscriber));
}

// push subscription data for the connection
async fn start_kafka_subscription_push(
    connection: WsConnection,
    mut kafka_receiver: Receiver<OwnedMessage>,
) {
    // receive kafka message in background and send them if possible
    info!(
        "Started to subscribe kafka data for peer: {}",
        connection.addr
    );
    // TODO: handle different topic and key
    while let Ok(msg) = kafka_receiver.recv().await {
        if Arc::strong_count(&connection.sender) <= 1 {
            break;
        }

        debug!("Receive a message: {:?}", &msg);

        if msg.payload().is_none() {
            warn!("Receive a kafka message whose payload is empty: {:?}", msg);
            continue;
        }

        match msg.topic() {
            // TODO: extract them as a trait
            "polkadot" | "kusama" => match msg.key() {
                Some(b"storage") => {
                    handle_kafka_storage(&connection, msg).await;
                }

                // TODO:
                Some(other) => {
                    warn!(
                        "Receive a message with key: `{}`",
                        String::from_utf8(other.to_vec()).expect("")
                    );
                }

                None => {
                    warn!("Receive a message without a key: {:?}", msg);
                }
            },
            // TODO:
            other => {
                warn!("Receive a message with topic: {:?}", other);
            }
        }
    }

    debug!("start_pushing_service return");
}

// TODO: refine these handles
async fn handle_kafka_storage(connection: &WsConnection, msg: OwnedMessage) {
    let storage_sessions = connection.storage_sessions.clone();
    let storage_sessions = storage_sessions.read().await;
    let res = collect_subscribed_storage(&storage_sessions, msg);
    let msgs = match res {
        Err(ref err) => {
            warn!(
                "Error occurred when collect data according to subscription params: {:?}",
                err
            );
            return;
        }

        Ok(msgs) => msgs,
    };

    let mut sender = connection.sender.lock().await;
    for msg in msgs.iter() {
        let msg = serde_json::to_string(msg).expect("serialize subscription data");
        let res = sender.feed(Message::Text(msg)).await;
        // TODO: refine it. We need to exit if connection is closed
        if let Err(Error::ConnectionClosed) = res {
            return;
        }
    }
    // TODO: need to handle ?
    let _res = sender.flush().await;
}

async fn handle_request(connection: WsConnection, subscriber: Arc<KVSubscriber>) {
    let mut receiver = connection.receiver.lock().await;
    loop {
        let msg = receiver.next().await;
        match msg {
            Some(Ok(msg)) => {
                let mut sender = connection.sender.lock().await;
                if msg.is_empty() {
                    continue;
                }
                match msg {
                    Message::Pong(_) => {}

                    Message::Binary(_) => {}

                    Message::Close(_) => {
                        // TODO: warn
                        break;
                    }

                    Message::Ping(_) => {
                        // TODO: need we to handle this?
                        let _res = sender.send(Message::Pong(b"pong".to_vec())).await;
                    }

                    Message::Text(s) => {
                        let res = connection.handle_message(s).await;
                        match res {
                            Ok(res) => {
                                let _res = sender.send(Message::Text(res)).await;

                                // TODO: remove kafka
                                // tokio::spawn(start_kafka_subscription_push(
                                //     connection.clone(),
                                //     subscriber.subscribe(),
                                // ));
                            }
                            Err(err) => {
                                let err = serde_json::to_string(
                                    &ResponseErrorMessage::from(err),
                                )
                                .expect("serialize a error response message");
                                // TODO: need we to handle this?
                                let _res = sender.send(Message::Text(err)).await;
                            }
                        };
                    }
                };
            }

            // closed connection
            Some(Err(Error::ConnectionClosed)) | None => break,

            Some(Err(err)) => {
                warn!("{}", err);
            }
        }
    }

    match connection.sender.lock().await.close().await {
        Ok(()) => {}
        Err(err) => {
            warn!(
                "Error occurred when closed connection to {}: {}",
                connection.addr, err
            );
        }
    };
}
