use crate::config::{Config, NodeConfig};
use crate::error::{Result, ServiceError};
use crate::message::{Failure, MethodCall, RequestMessage, Version};
use crate::polkadot::client::{
    handle_polkadot_response, polkadot_channel, MethodSenders,
};
use crate::polkadot::session::PolkadotSessions;
use crate::session::Session;

use log::*;
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};
use tokio::sync::{Mutex, RwLock};
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
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
    closed: Arc<AtomicBool>,
    cfg: Config,
    addr: SocketAddr,
    sender: Arc<Mutex<WsSender>>,
    receiver: Arc<Mutex<WsReceiver>>,
    polkadot_method_senders: MethodSenders,
    // TODO: split sessions from connection
    /// Polkadot related sessions
    pub polkadot_sessions: PolkadotSessions,
}

#[derive(Debug, Clone)]
pub struct WsConnections {
    inner: Arc<RwLock<HashMap<SocketAddr, WsConnection>>>,
}

impl WsConnections {
    pub fn new() -> Self {
        Default::default()
    }

    /// add an alive connection to pool
    pub async fn add(&mut self, conn: WsConnection) {
        if conn.closed() {
            return;
        }
        let mut map = self.inner.write().await;
        let expired = map.insert(conn.addr(), conn);
        Self::close_conn(expired).await
    }

    async fn close_conn(conn: Option<WsConnection>) {
        match conn {
            Some(conn) => {
                let _ = conn.close().await;
            }
            None => {}
        };
    }

    pub async fn remove(&mut self, addr: &SocketAddr) {
        let mut map = self.inner.write().await;
        let expired = map.remove(addr);
        Self::close_conn(expired).await
    }

    #[inline]
    pub fn inner(&self) -> Arc<RwLock<HashMap<SocketAddr, WsConnection>>> {
        self.inner.clone()
    }

    pub async fn len(&self) -> usize {
        self.inner.read().await.len()
    }
}

impl Default for WsConnections {
    fn default() -> Self {
        Self {
            inner: Arc::new(RwLock::new(Default::default())),
        }
    }
}

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

        // TODO: add node switch
        let (polkadot_method_senders, polkadot_method_receivers) = polkadot_channel();
        let conn = WsConnection {
            closed: Default::default(),
            cfg,
            addr,
            sender: Arc::new(Mutex::new(sender)),
            receiver: Arc::new(Mutex::new(receiver)),
            polkadot_sessions: Default::default(),
            polkadot_method_senders,
        };

        tokio::spawn(handle_polkadot_response(
            conn.clone(),
            polkadot_method_receivers,
        ));

        Ok(conn)
    }
}

impl WsConnection {
    #[inline]
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    #[inline]
    pub fn receiver(&self) -> Arc<Mutex<WsReceiver>> {
        self.receiver.clone()
    }

    #[inline]
    pub fn closed(&self) -> bool {
        self.closed.load(Ordering::Relaxed)
    }

    pub async fn close(&self) -> Result<()> {
        if self.closed() {
            return Ok(());
        }
        let mut sender = self.sender.lock().await;
        self.closed.store(true, Ordering::Relaxed);
        sender.close().await.map_err(ServiceError::WsServerError)
    }

    pub async fn send_close(&self) -> Result<()> {
        if self.closed() {
            return Ok(());
        }
        let res = self
            .sender
            .lock()
            .await
            .send(Message::Close(Some(CloseFrame {
                code: CloseCode::Normal,
                reason: Default::default(),
            })))
            .await;
        res.map_err(ServiceError::WsServerError)
    }

    pub async fn send_message(&self, msg: Message) -> Result<()> {
        if self.closed() {
            return Ok(());
        }
        let res = self.sender.lock().await.send(msg).await;
        res.map_err(ServiceError::WsServerError)
    }

    pub async fn send_messages(&self, msgs: Vec<Message>) -> Result<()> {
        if self.closed() {
            return Ok(());
        }
        let mut sender = self.sender.lock().await;
        for msg in msgs.into_iter() {
            sender.feed(msg).await?;
        }
        sender.flush().await.map_err(ServiceError::WsServerError)
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
                id: None,
            })
            .map_err(|err| {
                serde_json::to_string(&err).expect("serialize a failure message")
            })?;

        // handle different chain node
        match msg.chain.as_str() {
            crate::polkadot::consts::NODE_NAME => {
                self._handle_polkadot_message(session, request)
            }
            _ => Err(ServiceError::ChainNotSupport(msg.chain.clone()))
                .map_err(|err| err.to_string()),
        }
    }

    fn _handle_polkadot_message(
        &self,
        session: Session,
        request: MethodCall,
    ) -> std::result::Result<(), String> {
        let sender = self
            .polkadot_method_senders
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
            warn!("sender channel `{}` is closed", method);
        }

        Ok(())
    }

    /// Send successful response in other channel handler.
    /// The error result represents error occurred when send response
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
