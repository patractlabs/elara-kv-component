use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, Stream, StreamExt, TryStreamExt};
use log::*;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc};
use tokio::sync::{oneshot, Mutex};
use crate::message::MethodCall;
use message::{Id, Response, Value};
use jsonrpc_pubsub::SubscriptionId;
use std::collections::{BTreeMap, HashMap, HashSet};
use tokio::sync::broadcast::{Receiver, Sender};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::Duration;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Error, Result},
    MaybeTlsStream, WebSocketStream,
};
use url::Url;

type Subscription = mpsc::UnboundedSender<Value>;
type Subscriptions = Arc<Mutex<HashMap<SubscriptionId, Subscription>>>;
use crate::websocket::WsConnection;
use serde::{Deserialize, Serialize};
use std::borrow::BorrowMut;

/// A wrapper for WebSocketStream Client
#[derive(Clone)]
pub struct WsClient {
    pub sender:
        Arc<Mutex<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>>,
    // receiver: Arc<Mutex<SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>>>,

    // send received data to receiver
    ch_sender: Sender<Message>,

    resp_sender: Sender<WsResponse>,
    // sender: Sender<Message>,
    // receiver: Arc<Mutex<Receiver<Result<Message>>>>,
    addr: String,

    subscriptions: Subscriptions,
}

pub struct WsRpcClient {
    client: WsClient,
    manager: RpcManager,
    receiver: Receiver<WsResponse>,
}

impl WsRpcClient {
    pub async fn call(&mut self, call: &MethodCall) -> Result<WsResponse> {
        self.client.call(call).await?;

        let (sender, receiver) = oneshot::channel();
        self.manager.requests.insert(call.id.clone(), sender);
        self.sender
            .unbounded_send(Message::Text(request))
            .expect("Sending `Text` Message should be successful");

        let res = receiver.await;
        match res {
            Err(err) => Err(Error::AlreadyClosed),
            Ok(resp) => resp,
        }
    }

    pub async fn response(&mut self, id: &Id) -> Result<WsResponse> {
        let resp = self.receiver.recv().await;

        match resp {
            WsResponse::Response(resp) => {
                match resp {
                    Response::Single(output) => {
                        manager.finish_request(&output.id());
                        // TODO:
                    }
                    _ => {}
                }
            }
            WsResponse::Notification(notif) => {}
        };

        unimplemented!()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum WsResponse {
    Response(jsonrpc_core::Response),
    Notification(jsonrpc_core::Notification),
}

// async fn handle_ws_response(manager: &mut RpcManager, resp: WsResponse, connection: WsConnection) {
//     match resp {
//         WsResponse::Response(resp) => {
//             match resp {
//                 Response::Single(output ) => {
//                     manager.finish_request(&output.id());
//                     // TODO:
//                 },
//                 _ => {},
//             }
//         },
//         WsResponse::Notification(notif) => {},
//     };
// }
//

type Pending = oneshot::Sender<Result<WsResponse>>;
#[derive(Debug, Default)]
pub struct RpcManager {
    requests: HashMap<Id, Pending>,
    subscriptions: HashSet<SubscriptionId>,
}

impl WsClient {
    // /// Connect to a given URL.
    // pub async fn connect(addr: String) -> Result<Self> {
    //     let (mut socket, resp) =
    //         connect_async(Url::parse(&addr).expect("URL is invalid")).await?;
    //     let (mut sender, mut receiver) = socket.split();
    //
    //     let (ch_sender, ch_receiver) = mpsc::unbounded::<Message>();
    //     let (ch_sender2, ch_receiver2) = mpsc::unbounded::<Result<Message>>();
    //     tokio::spawn(ch_receiver.map(Ok).forward(sender));
    //     tokio::spawn(receiver.map(Ok).forward(ch_sender2));
    //
    //     Ok(Self {
    //         // sender: Arc::new(Mutex::new(sender)),
    //         receiver: Arc::new(Mutex::new(ch_receiver2)),
    //         sender: ch_sender,
    //         addr,
    //     })
    // }

    /// Connect to a given URL.
    pub async fn connect(addr: String) -> Result<Self> {
        let (mut socket, resp) =
            connect_async(Url::parse(&addr).expect("URL is invalid")).await?;
        let (mut sender, mut receiver) = socket.split();

        let (ch_sender, ch_receiver) = broadcast::channel(1000);
        let ch_sender_clone = ch_sender.clone();
        let (resp_sender, resp_receiver) = Self::start(ch_receiver);

        let client = Self {
            sender: Arc::new(Mutex::new(sender)),
            // receiver: Arc::new(Mutex::new(receiver)),
            ch_sender,
            addr,
            resp_sender,
            subscriptions: Default::default(),
        };

        // TODO: remove duplicated channel?
        tokio::spawn(async move {
            let next = receiver.next().await;
            match next {
                None => return,
                Some(Ok(msg)) => {
                    ch_sender_clone.send(msg);
                }
                Some(Err(err)) => {
                    warn!("Error occurred when receive a websocket message: {}", err);
                }
            }
        });

        Ok(client)
    }

    async fn reconnect(&mut self, err: Result<()>) {
        match err {
            // server closed?
            Err(Error::Io(err)) => {
                warn!("Cannot connect to server: {}", err);
                warn!("Try to connect to server later");
                while let Err(err) = self.try_reconnect().await {
                    tokio::time::sleep(Duration::from_secs(10)).await;
                }
            }

            Err(err) => {
                warn!("error occurred: {}", err);
                // we ignore other errors
            }

            Ok(()) => {}
        }
    }

    async fn try_reconnect(&mut self) -> Result<()> {
        let client = Self::connect(self.addr.clone()).await?;
        self.ch_sender = client.ch_sender;
        self.sender = client.sender;

        Ok(())
    }

    // TODO: remove
    pub fn subscribe(&self) -> Receiver<Message> {
        self.ch_sender.subscribe()
    }

    pub fn receiver(&mut self) -> Receiver<WsResponse> {
        self.resp_sender.subscribe()
    }

    /// start to receive response from server
    fn start(
        mut receiver: Receiver<Message>,
    ) -> (Sender<WsResponse>, Receiver<WsResponse>) {
        let (ch_sender, ch_receiver) = broadcast::channel(1000);

        let ch_sender_clone = ch_sender.clone();
        tokio::spawn(async move {
            while let Ok(msg) = receiver.recv().await {
                debug!("Receive a message: {:?}", &msg);
                match msg {
                    Message::Text(text) => {
                        let resp: WsResponse =
                            serde_json::from_str(&text).expect("Won't fail");
                        if let Err(_) = ch_sender_clone.send(resp) {
                            return;
                        }
                    }

                    // TODO:
                    Message::Binary(bytes) => {}

                    Message::Ping(bytes) => {}

                    Message::Close(closed) => {
                        return;
                    }

                    _ => {}
                }
            }
        });

        (ch_sender, ch_receiver)
    }

    pub async fn call(&mut self, request: &MethodCall) -> Result<()> {
        let request = serde_json::to_string(request).expect("Won't be failed");
        self.message(Message::Text(request)).await
    }

    pub fn addr(&self) -> String {
        self.addr.clone()
    }

    pub async fn ping(&mut self, bytes: impl Into<Vec<u8>>) -> Result<()> {
        let mut sender = self.sender.lock().await;
        sender.send(Message::Ping(bytes.into())).await
    }

    pub async fn message(&mut self, msg: impl Into<Message>) -> Result<()> {
        let mut sender = self.sender.lock().await;
        sender.send(msg.into()).await
    }

    pub async fn messages(&mut self, msgs: impl Into<Vec<Message>>) -> Result<()> {
        let mut sender = self.sender.lock().await;
        for msg in msgs.into().into_iter() {
            sender.feed(msg).await?;
        }
        sender.flush().await
    }
}
