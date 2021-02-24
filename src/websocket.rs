use std::{
    collections::HashMap,
    fmt, io,
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use futures::{
    sink::SinkExt,
    stream::{SplitSink, SplitStream, StreamExt},
};
use tokio::{
    net::{TcpListener, TcpStream, ToSocketAddrs},
    sync::{Mutex, RwLock},
};
use tokio_tungstenite::{
    accept_async,
    tungstenite::{
        self,
        protocol::{frame::coding::CloseCode, CloseFrame, Message},
    },
    WebSocketStream,
};

use crate::{
    cmd::ServiceConfig,
    message::{ElaraRequest, ElaraResponse, Error, Failure, MethodCall},
    session::Session,
    substrate, Chain,
};

/// A wrapper for WebSocketStream Server
#[derive(Debug)]
pub struct WsServer {
    listener: TcpListener,
}

impl WsServer {
    pub async fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        let listener = TcpListener::bind(&addr).await?;
        Ok(Self { listener })
    }

    /// returns a WebSocketStream and corresponding connection as a state
    pub async fn accept(&self, cfg: ServiceConfig) -> tungstenite::Result<WsConnection> {
        let (stream, addr) = self.listener.accept().await?;
        let stream = accept_async(stream).await?;
        let (sender, receiver) = stream.split();
        Ok(WsConnection {
            closed: Default::default(),
            config: cfg,
            addr,
            sender: Arc::new(Mutex::new(sender)),
            receiver: Arc::new(Mutex::new(receiver)),
            chain_handlers: Default::default(),
            sessions: Default::default(),
        })
    }
}

pub type WsSender = SplitSink<WebSocketStream<TcpStream>, Message>;
pub type WsReceiver = SplitStream<WebSocketStream<TcpStream>>;

/// Handle specified chain's subscription request
pub trait MessageHandler: Send + Sync {
    // TODO: refine the result type for better error handle

    /// When the result is Ok, it means to send it to corresponding subscription channel to handle it.
    /// When the result is Err, it means to response the err to peer.
    fn handle(&self, session: Session, request: MethodCall) -> Result<(), ElaraResponse>;
}

/// WsConnection maintains state. When WsServer accept a new connection, a WsConnection will be returned.
#[derive(Clone)]
pub struct WsConnection {
    closed: Arc<AtomicBool>,
    config: ServiceConfig,
    addr: SocketAddr,
    sender: Arc<Mutex<WsSender>>,
    receiver: Arc<Mutex<WsReceiver>>,
    chain_handlers: Arc<RwLock<HashMap<Chain, Box<dyn MessageHandler>>>>,
    pub sessions: ConnectionSessions,
}

impl fmt::Display for WsConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WsConnection(addr:{}, closed:{})",
            self.addr,
            self.closed.load(Ordering::SeqCst)
        )
    }
}

#[derive(Debug, Clone, Default)]
pub struct ConnectionSessions {
    pub polkadot_sessions: substrate::session::SubscriptionSessions,
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
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    pub fn close(&self) {
        if self.is_closed() {
            return;
        }
        self.closed.store(true, Ordering::SeqCst);
    }

    pub async fn send_close(&self) -> tungstenite::Result<()> {
        if self.is_closed() {
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

    pub async fn register_message_handler(
        &mut self,
        chain_name: Chain,
        handler: impl MessageHandler + 'static,
    ) -> &mut Self {
        self.chain_handlers
            .write()
            .await
            .insert(chain_name, Box::new(handler));
        self
    }

    /// Send successful response in other channel handler.
    /// The error result represents error occurred when send response
    pub async fn handle_message(&self, msg: impl Into<String>) -> tungstenite::Result<()> {
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

    // when result is ok, it means to send it to corresponding subscription channel to handle it.
    // When result is err, it means to response the err to peer.
    // The error is elara error code, not jsonrpc error.
    async fn _handle_message(&self, msg: impl Into<String>) -> Result<(), ElaraResponse> {
        let msg = msg.into();
        let msg = serde_json::from_str::<ElaraRequest>(msg.as_str())
            .map_err(|_| ElaraResponse::failure(None, None, Error::parse_error()))?;

        // validate node name
        if !self.config.validate(&msg.chain) {
            return Err(ElaraResponse::failure(
                Some(msg.id.clone()),
                Some(msg.chain),
                Error::parse_error(),
            ));
        }

        let session: Session = Session::from(&msg);
        let request = serde_json::from_str::<MethodCall>(&msg.request)
            .map_err(|_| Failure::new(Error::parse_error(), None))
            .map_err(|err| serde_json::to_string(&err).expect("serialize a failure message"))
            .map_err(|res| ElaraResponse::success(msg.id.clone(), msg.chain, res))?;

        let chain_handlers = self.chain_handlers.read().await;

        let handler = chain_handlers.get(&msg.chain);

        let handler = handler
            .ok_or_else(Error::parse_error)
            .map_err(|err| ElaraResponse::failure(Some(msg.id.clone()), Some(msg.chain), err))?;
        handler.handle(session, request)
    }
}

#[derive(Clone, Default)]
pub struct WsConnections {
    // client addr ==> ws connection
    inner: Arc<RwLock<HashMap<SocketAddr, WsConnection>>>,
}

impl WsConnections {
    pub fn new() -> Self {
        Self::default()
    }

    /// add an alive connection to pool
    pub async fn add(&mut self, conn: WsConnection) -> &mut Self {
        if !conn.is_closed() {
            let mut map = self.inner.write().await;
            let expired = map.insert(conn.addr(), conn);
            Self::close(expired);
        }
        self
    }

    /// remove an alive connection from pool and close it
    pub async fn remove(&mut self, addr: &SocketAddr) -> &mut Self {
        {
            let mut map = self.inner.write().await;
            let expired = map.remove(addr);
            Self::close(expired);
        }
        self
    }

    fn close(conn: Option<WsConnection>) {
        if let Some(conn) = conn {
            conn.close();
        }
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
