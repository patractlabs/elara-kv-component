use crate::config::{Config, NodeConfig};
use crate::message::{
    ErrorMessage, Failure, MethodCall, RequestMessage, ResponseMessage, Version,
};
use crate::polkadot;
use crate::session::Session;

use core::result;
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
    ) -> std::result::Result<(), ResponseMessage>;
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
        Self::close_conn(expired);
    }

    fn close_conn(conn: Option<WsConnection>) {
        if let Some(conn) = conn {
            conn.close();
        }
    }
    /// remove an alive connection from pool and close it
    pub async fn remove(&mut self, addr: &SocketAddr) {
        let mut map = self.inner.write().await;
        let expired = map.remove(addr);
        Self::close_conn(expired);
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

fn validate_chain(
    nodes: &HashMap<String, NodeConfig>,
    chain: &str,
) -> result::Result<(), ErrorMessage> {
    if nodes.contains_key(chain) {
        Ok(())
    } else {
        Err(ErrorMessage::chain_not_found())
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
        self.closed.load(Ordering::SeqCst)
    }

    pub fn close(&self) {
        if self.closed() {
            return;
        }
        self.closed.store(true, Ordering::SeqCst);
    }

    pub async fn send_close(&self) -> tungstenite::Result<()> {
        if self.closed() {
            return Ok(());
        }
        self.sender
            .lock()
            .await
            .send(Message::Close(Some(CloseFrame {
                code: CloseCode::Normal,
                reason: Default::default(),
            })))
            .await
    }

    pub async fn send_message(&self, msg: Message) -> tungstenite::Result<()> {
        self.sender.lock().await.send(msg).await
    }

    pub async fn send_messages(&self, msgs: Vec<Message>) -> tungstenite::Result<()> {
        let mut sender = self.sender.lock().await;
        for msg in msgs.into_iter() {
            sender.feed(msg).await?;
        }
        sender.flush().await
    }

    // when result is ok, it means to send it to corresponding subscription channel to handle it.
    // When result is err, it means to response the err to peer.
    // The error is elara error code, not jsonrpc error.
    async fn _handle_message(
        &self,
        msg: impl Into<String>,
    ) -> std::result::Result<(), ResponseMessage> {
        let msg = msg.into();
        let msg: RequestMessage = serde_json::from_str(msg.as_str()).map_err(|_| {
            ResponseMessage::error_response(None, None, ErrorMessage::parse_error())
        })?;

        validate_chain(&self.cfg.nodes, &msg.chain).map_err(|err| {
            ResponseMessage::error_response(
                Some(msg.id.clone()),
                Some(msg.chain.clone()),
                err,
            )
        })?;

        let session: Session = Session::from(&msg);
        let request = serde_json::from_str::<MethodCall>(&msg.request)
            .map_err(|_| Failure {
                jsonrpc: Version::V2_0,
                error: jsonrpc_types::Error::parse_error(),
                id: None,
            })
            .map_err(|err| {
                serde_json::to_string(&err).expect("serialize a failure message")
            })
            .map_err(|res| ResponseMessage {
                id: Some(msg.id.clone()),
                chain: Some(msg.chain.clone()),
                error: None,
                result: Some(res),
            })?;

        let chain_handlers = self.chain_handlers.read().await;

        let handler = chain_handlers.get(msg.chain.as_str());

        let handler =
            handler
                .ok_or_else(ErrorMessage::chain_not_found)
                .map_err(|err| {
                    ResponseMessage::error_response(
                        Some(msg.id.clone()),
                        Some(msg.chain.clone()),
                        err,
                    )
                })?;
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
    pub async fn handle_message(
        &self,
        msg: impl Into<String>,
    ) -> tungstenite::Result<()> {
        let res = self._handle_message(msg).await;
        match res {
            // send no rpc error response in here
            Err(resp) => {
                self.send_message(Message::Text(
                    serde_json::to_string(&resp).expect("serialize a response message"),
                ))
                .await
            }
            // do other rpc logic in other way
            Ok(()) => Ok(()),
        }
    }
}
