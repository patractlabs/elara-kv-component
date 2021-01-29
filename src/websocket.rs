use crate::error::{Result, ServiceError};
use crate::kafka_api::KafkaStoragePayload;
use crate::message::{
    Failure, Id, MethodCall, RequestMessage, ResponseMessage, SubscribedData,
    SubscribedMessage, SubscribedParams, Success, Version,
};
use crate::rpc_api::SubscribedResult;
use crate::session::{ChainSessions, Session, StorageKeys, StorageSessions};
use crate::polkadot::util;
use crate::polkadot::session::PolkadotSessions;
use rdkafka::message::OwnedMessage;
use rdkafka::Message as KafkaMessage;

use crate::rpc_api::state::*;
use crate::polkadot::util::{StorageSubscriber, Subscriber, polkadot_channel, MethodSender, MethodReceiver, MethodSenders, MethodReceivers};
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use log::*;
use std::net::SocketAddr;
use std::ops::DerefMut;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};
use tokio::sync::{Mutex, RwLock};
use tokio_tungstenite::{accept_async, tungstenite, WebSocketStream};
use tungstenite::Message;

use crate::config::{Config, NodeConfig};
use std::collections::HashMap;
use std::borrow::BorrowMut;

/// A wrapper for WebSocketStream Server
#[derive(Debug)]
pub struct WsServer {
    listener: TcpListener,
}

/// WsConnection maintains state. When WsServer accept a new connection, a WsConnection will be returned.
#[derive(Debug, Clone)]
pub struct WsConnection {
    cfg: Config,
    pub addr: SocketAddr,
    pub sender: Arc<Mutex<WsSender>>,
    pub receiver: Arc<Mutex<WsReceiver>>,

    pub polkadot_sessions: PolkadotSessions,

    // pub storage_sessions: Arc<RwLock<StorageSessions>>,
    // pub chain_sessions: Arc<RwLock<ChainSessions>>,

    method_senders: MethodSenders,
}

// TODO: 增加一个API，在连接刚启动的时候指定这个连接可以订阅哪些节点

pub type WsSender = SplitSink<WebSocketStream<TcpStream>, Message>;
pub type WsReceiver = SplitStream<WebSocketStream<TcpStream>>;

fn validate_chain(nodes: &HashMap<String, NodeConfig>, chain: &String) -> Result<()> {
    if nodes.contains_key(chain) {
        Ok(())
    } else {
        Err(ServiceError::ChainNotSupport(chain.clone()))
    }
}

impl WsServer {
    pub async fn bind<A: ToSocketAddrs>(addr: A) -> std::io::Result<Self> {
        let listener = TcpListener::bind(&addr).await?;

        Ok(Self { listener })
    }

    pub fn sender(&self) -> Arc<RwLock<WsSender>> {
        self.sender()
    }

    pub fn receiver(&self) -> Arc<RwLock<WsReceiver>> {
        self.receiver()
    }

    /// returns a WebSocketStream and corresponding connection as a state
    pub async fn accept(&self, cfg: Config) -> tungstenite::Result<WsConnection> {
        let (stream, addr) = self.listener.accept().await?;
        let stream = accept_async(stream).await?;
        let (sender, receiver) = stream.split();

        // TODO: config
        let (method_senders, method_receivers) = polkadot_channel();


        Ok(WsConnection {
            cfg,
            addr,
            sender: Arc::new(Mutex::new(sender)),
            receiver: Arc::new(Mutex::new(receiver)),
            polkadot_sessions: Default::default(),
            // storage_sessions: Arc::new(RwLock::new(StorageSessions::new())),
            // chain_sessions: Arc::new(RwLock::new(ChainSessions::new())),
            method_senders,
        })
    }
}

// we start to spawn handler task in background to response subscription method
async fn handle_message_background(conn: WsConnection, receivers: MethodReceivers) {
    for (method, mut receiver) in receivers.into_iter() {
        let conn = conn.clone();

        // match method {
        //
        // }
        tokio::spawn(async move {
            while let Some(request) = receiver.recv().await {
                // TODO:
            }
        });
    }
}


async fn handle_state_subscribeStorage(ws_sender: Arc<Mutex<WsSender>>, sessions: PolkadotSessions, mut receiver: MethodReceiver) {
    while let Some(request) = receiver.recv().await {
        let mut sessions = sessions.storage_sessions.write().await;
        util::handle_state_subscribeStorage(sessions.deref_mut());

        let sender = ws_sender.lock().await;
    }
}


impl WsConnection {

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub async fn send_message(&self, msg: Message) -> Result<()> {
        let res = self.sender.lock().await.send(msg).await;
        res.map_err(ServiceError::WsClientError)
    }

    pub async fn send_messages(&self, msgs: Vec<Message>) -> Result<()> {
        let mut sender = self.sender.lock().await;
        for msg in msgs.into_iter() {
            sender.feed(msg).await?;
        }
        sender.flush().await.map_err(ServiceError::WsClientError)
    }

    pub async fn handle_message(&self, msg: impl Into<String>) -> Result<String> {
        let msg = msg.into();
        let msg: Result<RequestMessage> =
            serde_json::from_str(&*msg).map_err(ServiceError::JsonError);
        match msg {
            Ok(msg) => {
                // handle jsonrpc error
                validate_chain(&self.cfg.nodes, &msg.chain)?;
                let session: Session = Session::from(&msg);
                let res = self.handle_request(session.clone(), msg).await;
                let result = match res {
                    Ok(success) => serde_json::to_string(&success)
                        .expect("serialize result for success response"),
                    Err(failure) => serde_json::to_string(&failure)
                        .expect("serialize result for failure response"),
                };

                let response = ResponseMessage {
                    id: session.client_id,
                    chain: session.chain_name,
                    result,
                };

                Ok(serde_json::to_string(&response)
                    .expect("serialize response for elara"))
            }
            Err(err) => Err(err),
        }
    }

    // if receive a method call that is not a legal jsonrpc, we return failure directly.
    async fn handle_request(
        &self,
        session: Session,
        msg: RequestMessage,
    ) -> std::result::Result<Success, Failure> {
        let request =
            serde_json::from_str::<MethodCall>(&*msg.request).map_err(|_| Failure {
                jsonrpc: Version::V2_0,
                error: jsonrpc_types::Error::parse_error(),
                // TODO: need to be null
                id: Id::Num(0),
            })?;




        //
        // let id = request.id.clone();
        // let storage_sessions = self.storage_sessions.clone();
        // let mut storage_sessions = storage_sessions.write().await;
        //
        // // TODO: use hashmap rather than if-else
        // let res = if request.method == *"state_subscribeStorage" {
        //     let mut sub = StorageSubscriber {
        //         sessions: storage_sessions.deref_mut(),
        //         session,
        //     };
        //
        //     sub.subscribe(request)
        //
        // // util::handle_state_subscribeStorage(
        // //     storage_sessions.deref_mut(),
        // //     session,
        // //     request,
        // // )
        // } else if request.method == *"state_unsubscribeStorage" {
        //     util::handle_state_unsubscribeStorage(
        //         storage_sessions.deref_mut(),
        //         session,
        //         request,
        //     )
        // } else {
        //     Err(jsonrpc_types::Error::method_not_found())
        // };
        //
        // res.map_err(|err| Failure {
        //     jsonrpc: Version::V2_0,
        //     error: err,
        //     id,
        // })
    }
}

// transfer a kafka message to a group of SubscribedMessage according to storage session
pub fn collect_subscribed_storage(
    sessions: &StorageSessions,
    msg: OwnedMessage,
) -> Result<Vec<SubscribedMessage>> {
    // ignore msg which does not have a payload
    let payload = match msg.payload() {
        Some(payload) => payload,
        _ => unreachable!(),
    };
    let payload: KafkaStoragePayload =
        serde_json::from_slice(payload).map_err(ServiceError::JsonError)?;

    // result for subscribing all storages
    let result: SubscribedResult = StateStorageResult::from(&payload).into();

    let mut msgs = vec![];
    for (subscription_id, (session, storage)) in sessions.iter() {
        let data: String = match storage {
            // send the subscription data to this subscription unconditionally
            StorageKeys::All => {
                let data = SubscribedData {
                    jsonrpc: Version::V2_0,
                    // TODO:
                    method: "".to_string(),
                    params: SubscribedParams {
                        // TODO: make sure the subscription could be client_id.
                        result: result.clone(),
                        subscription: subscription_id.clone(),
                    },
                };

                serde_json::to_string(&data).expect("serialize a subscribed data")
            }

            // TODO: do filter for keys
            StorageKeys::Some(_keys) => {
                unimplemented!();
            }
        };

        msgs.push(SubscribedMessage {
            id: session.client_id.clone(),
            chain: session.chain_name.clone(),
            data,
        });
    }

    Ok(msgs)
}
