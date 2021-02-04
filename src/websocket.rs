use crate::config::{Config, NodeConfig};
use crate::error::{Result, ServiceError};
use crate::message::{Failure, MethodCall, RequestMessage, Version};
use crate::polkadot;
use crate::session::Session;

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
#[derive(Clone)]
pub struct WsConnection {
    closed: Arc<AtomicBool>,
    cfg: Config,
    addr: SocketAddr,
    sender: Arc<Mutex<WsSender>>,
    receiver: Arc<Mutex<WsReceiver>>,
    chain_handlers: Arc<RwLock<HashMap<&'static str, Box<dyn MessageHandler>>>>,

    pub sessions: ConnectionSessions,
}

#[derive(Clone)]
pub struct WsConnections {
    inner: Arc<RwLock<HashMap<SocketAddr, WsConnection>>>,
}

#[derive(Debug, Clone, Default)]
pub struct ConnectionSessions {
    pub polkadot_sessions: polkadot::session::SubscriptionSessions,
}

/// Handle specified chain's subscription request
pub trait MessageHandler: Send + Sync {
    // TODO: refine the result type for better error handle

    /// When the result is Ok, it means to send it to corresponding subscription channel to handle it.
    /// When the result is Err, it means to response the err to peer.
    fn handle(
        &self,
        session: Session,
        request: MethodCall,
    ) -> std::result::Result<(), String>;
}

pub type WsSender = SplitSink<WebSocketStream<TcpStream>, Message>;
pub type WsReceiver = SplitStream<WebSocketStream<TcpStream>>;

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
        if let Some(conn) = conn {
            let _ = conn.close().await;
        }
    }
    /// remove an alive connection from pool and close it
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

    pub async fn is_empty(&self) -> bool {
        self.inner.read().await.is_empty()
    }
}

impl Default for WsConnections {
    fn default() -> Self {
        Self {
            inner: Arc::new(RwLock::new(Default::default())),
        }
    }
}

fn validate_chain(nodes: &HashMap<String, NodeConfig>, chain: &str) -> Result<()> {
    if nodes.contains_key(chain) {
        Ok(())
    } else {
        Err(ServiceError::ChainNotSupport(chain.to_string()))
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

        let conn = WsConnection {
            closed: Default::default(),
            cfg,
            addr,
            sender: Arc::new(Mutex::new(sender)),
            receiver: Arc::new(Mutex::new(receiver)),
            chain_handlers: Default::default(),
            sessions: Default::default(),
        };
        Ok(conn)
    }
}

impl WsConnection {
    /// Peer user ip:port address
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
    async fn _handle_message(
        &self,
        msg: impl Into<String>,
    ) -> std::result::Result<(), String> {
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

        let chain_handlers = self.chain_handlers.read().await;

        let handler = chain_handlers.get(msg.chain.as_str());

        let handler = handler
            .ok_or_else(|| ServiceError::ChainNotSupport(msg.chain.clone()))
            .map_err(|err| err.to_string())?;
        handler.handle(session, request)
    }

    pub async fn register_message_handler(
        &mut self,
        chain_name: &'static str,
        handler: impl MessageHandler + 'static,
    ) {
        self.chain_handlers
            .write()
            .await
            .insert(chain_name, Box::new(handler));
    }

    /// Send successful response in other channel handler.
    /// The error result represents error occurred when send response
    pub async fn handle_message(&self, msg: impl Into<String>) -> Result<()> {
        let res = self._handle_message(msg).await;
        match res {
            // send no rpc error response in here
            Err(err) => self.send_message(Message::Text(err)).await,
            // do other rpc logic in other way
            Ok(()) => Ok(()),
        }
    }
}
