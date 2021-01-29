use crate::config::{Config, NodeConfig};
use crate::error::{Result, ServiceError};
use crate::message::{
    Failure, Id, MethodCall, RequestMessage, ResponseMessage, SubscribedData,
    SubscribedMessage, SubscribedParams, Success, Version,
};
use crate::polkadot::session::PolkadotSessions;
use crate::polkadot::util;
use crate::polkadot::util::{
    polkadot_channel, MethodReceiver, MethodReceivers, MethodSender, MethodSenders,
    StorageSubscriber, Subscriber,
};
use crate::rpc_api::state::*;
use crate::rpc_api::SubscribedResult;
use crate::session::{Session, StorageKeys, StorageSessions};

use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use log::*;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::ops::DerefMut;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};
use tokio::sync::{Mutex, RwLock};
use tokio_tungstenite::{accept_async, tungstenite, WebSocketStream};
use tungstenite::Message;

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

    /// returns a WebSocketStream and corresponding connection as a state
    pub async fn accept(&self, cfg: Config) -> tungstenite::Result<WsConnection> {
        let (stream, addr) = self.listener.accept().await?;
        let stream = accept_async(stream).await?;
        let (sender, receiver) = stream.split();

        // TODO: config
        let (method_senders, _method_receivers) = polkadot_channel();

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
    for (_method, mut receiver) in receivers.into_iter() {
        let _conn = conn.clone();

        // TODO:
        tokio::spawn(async move {
            while let Some(_request) = receiver.recv().await {
                // TODO:
            }
        });
    }
}

async fn handle_state_subscribeStorage(
    ws_sender: Arc<Mutex<WsSender>>,
    sessions: PolkadotSessions,
    mut receiver: MethodReceiver,
) {
    while let Some((session, request)) = receiver.recv().await {
        let mut sessions = sessions.storage_sessions.write().await;
        util::handle_state_subscribeStorage(sessions.deref_mut(), session, request);

        let _sender = ws_sender.lock().await;
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

    // when result is ok, it means to send it to corresponding subscription channel to handle it.
    // When result is err, it means to response the err to peer.
    // The error is elara error code, not jsonrpc error.
    fn _handle_message(&self, msg: impl Into<String>) -> std::result::Result<(), String> {
        let msg = msg.into();
        let msg: RequestMessage =
            serde_json::from_str(msg.as_str()).map_err(|err| err.to_string())?;

        validate_chain(&self.cfg.nodes, &msg.chain).map_err(|err| err.to_string())?;
        let session: Session = Session::from(&msg);
        let request = serde_json::from_str::<MethodCall>(&msg.request)
            .map_err(|_| Failure {
                jsonrpc: Version::V2_0,
                error: jsonrpc_types::Error::parse_error(),
                // TODO: need to be null
                id: None,
            })
            .map_err(|err| {
                serde_json::to_string(&err).expect("serialize a failure message")
            })?;

        let sender = self
            .method_senders
            .get(request.method.as_str())
            .ok_or(Failure {
                jsonrpc: Version::V2_0,
                error: jsonrpc_types::Error::method_not_found(),
                id: Some(request.id.clone()),
            })
            .map_err(|err| {
                serde_json::to_string(&err).expect("serialize a failure message")
            })?;

        let method = request.method.clone();
        let res = sender.send((session, request));
        if res.is_err() {
            warn!("sender about `{}` is closed", method);
        }

        Ok(())
    }

    // send successful response in other channel handler
    pub async fn handle_message(&self, msg: impl Into<String>) -> Result<()> {
        let res = self._handle_message(msg);
        match res {
            // send no rpc error response in here
            Err(err) => self.send_message(Message::Text(err)).await,
            // do other rpc logic in other way
            Ok(()) => Ok(()),
        }
    }
}
