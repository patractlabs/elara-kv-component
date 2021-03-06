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

use crate::compression::{Encoder, GzipEncoder, ZlibEncoder};
use crate::message::{CompressionType, ConfigRequest, ElaraRequest};
use crate::rpc_client::ArcRpcClient;
use crate::substrate::session::SubscriptionSessions;
use crate::{
    cmd::ServiceConfig,
    message::{ElaraResponse, Error, Failure, MethodCall, SubscriptionRequest},
    rpc_client::RpcClients,
    session::Session,
    substrate, Chain,
};
use flate2::Compression;
use std::sync::atomic::AtomicUsize;

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
    pub async fn accept(
        &self,
        clients: RpcClients,
        cfg: ServiceConfig,
    ) -> tungstenite::Result<WsConnection> {
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
            clients,
            chains: Default::default(),
            compression: Default::default(),
        })
    }
}

pub type WsSender = SplitSink<WebSocketStream<TcpStream>, Message>;
pub type WsReceiver = SplitStream<WebSocketStream<TcpStream>>;

/// Handle specified chain's subscription request
pub trait MessageHandler: Send + Sync {
    fn chain(&self) -> Chain;

    // start response task in background. Can be called only once.
    fn handle_response(&mut self, conn: WsConnection, sessions: SubscriptionSessions);

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
    clients: RpcClients,
    chains: Arc<RwLock<HashMap<Chain, substrate::session::SubscriptionSessions>>>,
    compression: Arc<AtomicUsize>,
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

    #[inline]
    pub fn config(&self) -> &ServiceConfig {
        &self.config
    }

    #[inline]
    pub fn compression_type(&self) -> CompressionType {
        self.compression.load(Ordering::SeqCst).into()
    }

    #[inline]
    pub fn set_compression_type(&self, ty: CompressionType) {
        self.compression.store(ty as usize, Ordering::SeqCst)
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

    #[inline]
    pub async fn send_text(&self, text: String) -> tungstenite::Result<()> {
        self.send_message(Message::Text(text)).await
    }

    #[inline]
    pub async fn send_binary(&self, bytes: Vec<u8>) -> tungstenite::Result<()> {
        self.send_message(Message::Binary(bytes)).await
    }

    /// If compression is none, it send the original text with text type.
    /// If compression is other, it send the encoded text with binary type and encode type prefix 4 bytes string.
    pub async fn send_compression_data(&self, text: String) -> tungstenite::Result<()> {
        match self.compression_type() {
            CompressionType::None => self.send_text(text).await,
            CompressionType::Gzip => {
                // According to actual test, the compression rate is about 32% ~ 50%
                let mut bytes = Vec::with_capacity((text.len() >> 1) + (text.len() >> 2));
                bytes.extend_from_slice(b"gzip");
                let text_len = text.len();
                let bytes = GzipEncoder::new(Compression::fast())
                    .encode(&text, bytes)
                    .expect("encode gzip data");
                log::debug!(
                    "gzip: {}, {}, {}",
                    text_len,
                    bytes.len(),
                    bytes.len() as f64 / text_len as f64
                );
                self.send_binary(bytes).await
            }

            CompressionType::Zlib => {
                // According to actual test, the compression rate is about 30 ~ 45%
                let mut bytes = Vec::with_capacity((text.len() >> 1) + (text.len() >> 2));
                bytes.extend_from_slice(b"zlib");
                let text_len = text.len();
                let bytes = ZlibEncoder::new(Compression::best())
                    .encode(&text, bytes)
                    .expect("encode zlib data");
                log::debug!(
                    "zlib: {}, {}, {}",
                    text_len,
                    bytes.len(),
                    bytes.len() as f64 / text_len as f64
                );
                self.send_binary(bytes).await
            }
        }
    }

    pub async fn register_message_handler(
        &mut self,
        chain_name: Chain,
        mut handler: impl MessageHandler + 'static,
    ) {
        let mut chains = self.chains.write().await;

        let sessions = SubscriptionSessions::default();
        chains.insert(chain_name.clone(), sessions.clone());
        handler.handle_response(self.clone(), sessions);
        self.chain_handlers
            .write()
            .await
            .insert(chain_name.clone(), Box::new(handler));
    }

    pub async fn get_sessions(&self, chain: &Chain) -> Option<SubscriptionSessions> {
        let chains = self.chains.read().await;
        chains.get(chain).cloned()
    }

    pub fn get_client(&self, chain: &Chain) -> Option<ArcRpcClient> {
        self.clients.get(chain).cloned()
    }

    /// Send successful response in other channel handler.
    /// The error result represents error occurred when send response
    pub async fn handle_message(&self, msg: impl Into<String>) -> tungstenite::Result<()> {
        const EXPECT: &str = "serialize a response message";
        let msg = msg.into();
        let req = serde_json::from_str::<ElaraRequest>(msg.as_str())
            .map_err(|err| ElaraResponse::failure(None, None, Error::invalid_params(err)));

        let req = match req {
            Ok(req) => req,
            Err(resp) => {
                self.send_text(serde_json::to_string(&resp).expect(EXPECT))
                    .await?;
                return Ok(());
            }
        };

        match req {
            ElaraRequest::SubscriptionRequest(req) => {
                let resp = self.handle_subscription_request(req).await;
                match resp {
                    // send no rpc error response in here
                    Err(resp) => {
                        self.send_text(serde_json::to_string(&resp).expect(EXPECT))
                            .await?;
                    }
                    // if ok, handle the request in other task
                    Ok(()) => {}
                }
            }
            ElaraRequest::ConfigRequest(cfg) => {
                let resp = self.handle_config_request(cfg).await;
                self.send_text(serde_json::to_string(&resp).expect(EXPECT))
                    .await?;
            }

            ElaraRequest::UnknownRequest(req) => {
                self.send_text(
                    serde_json::to_string(&ElaraResponse::failure(
                        req.id,
                        None,
                        Error::invalid_request(),
                    ))
                    .expect(EXPECT),
                )
                .await?;
            }
        }

        Ok(())
    }

    async fn handle_config_request(&self, cfg: ConfigRequest) -> ElaraResponse {
        self.set_compression_type(cfg.compression);
        ElaraResponse::config_response(cfg.id, None)
    }

    // when result is ok, it means to send it to corresponding subscription channel to handle it.
    // When result is err, it means to response the err to peer.
    // The error is elara error code, not jsonrpc error.
    async fn handle_subscription_request(
        &self,
        msg: SubscriptionRequest,
    ) -> Result<(), ElaraResponse> {
        // validate node name
        if !self.config.validate(&msg.chain) {
            return Err(ElaraResponse::failure(
                Some(msg.id.clone()),
                Some(msg.chain.clone()),
                Error::invalid_params(format!("Chain `{}` is not supported", &msg.chain)),
            ));
        }

        let session: Session = Session::from(&msg);
        let request = serde_json::from_str::<MethodCall>(&msg.request)
            .map_err(|_| Failure::new(Error::parse_error(), None))
            .map_err(|err| serde_json::to_string(&err).expect("serialize a failure message"))
            .map_err(|res| ElaraResponse::success(msg.id.clone(), msg.chain.clone(), res))?;

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
