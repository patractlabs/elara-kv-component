#[macro_use]
extern crate lazy_static;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use async_jsonrpc_client::{
    MethodCall, Notification, NotificationStream, PubsubTransport, Transport,
};
use async_jsonrpc_client::RpcClientError;
use async_jsonrpc_client::WsTransport;
use futures::{Stream, TryStreamExt};
use futures::channel::mpsc::UnboundedReceiver;
use jsonrpc_types::{Call, Id, Params, Success, Version};
use log::*;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio_stream::StreamExt;
use web3::transports::WebSocket;

pub use error::ServiceError;

use crate::message::{SubscriptionId, Value};
use crate::polkadot::send_state_storage;
use crate::websocket::WsConnection;

// pub mod client;
pub mod config;
pub mod error;
pub mod kafka;
pub mod message;
// pub mod service;
pub mod polkadot;
pub mod session;
pub mod websocket;

mod kafka_api;
mod rpc_api;

// use web3::Result;
// use web3::{DuplexTransport, Transport};

pub type Result<T, E = RpcClientError> = std::result::Result<T, E>;

const state_subscribeStorage: &str = "state_subscribeStorage";
const state_unsubscribeStorage: &str = "state_unsubscribeStorage";
const state_subscribeRuntimeVersion: &str = "state_subscribeRuntimeVersion";
const state_unsubscribeRuntimeVersion: &str = "state_unsubscribeRuntimeVersion";

const chain_subscribeAllHeads: &str = "chain_subscribeAllHeads";
const chain_unsubscribeAllHeads: &str = "chain_unsubscribeAllHeads";
const chain_subscribeNewHeads: &str = "chain_subscribeNewHeads";
const chain_unsubscribeNewHeads: &str = "chain_unsubscribeNewHeads";
const chain_subscribeFinalizedHeads: &str = "chain_subscribeFinalizedHeads";
const chain_unsubscribeFinalizedHeads: &str = "chain_unsubscribeFinalizedHeads";

const author_submitAndWatchExtrinsic: &str = "author_submitAndWatchExtrinsic";
const author_unwatchExtrinsic: &str = "author_unwatchExtrinsic";

pub struct RpcClient {
    ws: WsTransport,
}

/// Successful response with subscription id
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubscribedSuccess {
    /// Protocol version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jsonrpc: Option<Version>,
    /// Result
    pub result: SubscriptionId,
    /// Correlation id
    pub id: Id,
}

// #[derive(Error, Debug)]
// pub enum SubscriptionError {
//     #[error(transparent)]
//     JsonrpcError(#[from] jsonrpc_core::Error),
//
//     #[error(transparent)]
//     JsonError(#[from] serde_json::Error),
// }
//

// TODO: use different struct for different chain node
pub struct SubscribedStream {
    // TODO: define their own struct
    storage_stream: NotificationStream,
    version_stream: NotificationStream,
    head_stream: NotificationStream,
}
// 现在是收到的数据会发往所有的连接，而不区分连接某个订阅本身订阅的节点类型
async fn send_messages_to_conns<T, S>(
    mut stream: S,
    conns: Arc<Mutex<HashMap<SocketAddr, WsConnection>>>,
    // Do send logic for one connection.
    // It should be non-blocking
    sender: fn(addr: SocketAddr, WsConnection, T),
) where
    T: Serialize + Clone,
    S: Unpin + Stream<Item = T>,
{
    while let Some(data) = stream.next().await {
        // we get a new data then we send it to all conns
        for (addr, conn) in conns.lock().await.iter() {
            // send_message_to_conn(addr.clone(), conn.clone(), data.clone(), sender);
            let addr = *addr;
            let conn = conn.clone();
            // send data to conns
            sender(addr, conn, data.clone());
        }
    }
}

async fn send_message_to_conn<T>(
    addr: SocketAddr,
    conn: WsConnection,
    data: T,
    // Do send logic for one connection.
    // It should be non-blocking
    sender: fn(addr: SocketAddr, WsConnection, T),
) where
    T: Serialize + Clone,
{
    // send data to conns
    sender(addr, conn, data);
}

impl SubscribedStream {
    // start to push subscription data to all connections in background
    pub fn start_send_messages_to_conns(
        self,
        conns: Arc<Mutex<HashMap<SocketAddr, WsConnection>>>,
    ) {
        let Self {
            storage_stream,
            version_stream: _,
            head_stream: _,
        } = self;
        tokio::spawn(send_messages_to_conns(
            storage_stream,
            conns,
            |addr, conn, data| {
                // TODO:
                match serde_json::value::from_value(data.params.result.clone()) {
                    Ok(data) => send_state_storage(addr, conn, data),

                    Err(_err) => {
                        warn!("Receive a illegal subscribed data: {}", &data)
                    }
                };
            },
        ));

        // TODO: spawn other subscription
    }

    // pub async fn send_messages_to_conn(this: Arc<Mutex<Self>>, conn: WsConnection) {
    //     tokio::spawn(async move {
    //         let mut stream = stream.lock().await;
    //         stream.send_state_storage()
    //     });
    // }

    // async fn send_state_storage(&mut self, conns: WsConnection) {
    //     while let Some(storage) = self.storage_stream.next().await {
    //         // TODO: use storage struct
    //         // TODO: send data
    //     }
    // }
}

impl RpcClient {
    pub async fn new(addr: impl Into<String>) -> Result<Self> {
        let ws = WsTransport::new(addr.into().as_str()).await?;

        Ok(Self { ws })
    }

    // TODO: add config for different chain node
    pub async fn start(&self) -> Result<SubscribedStream> {
        let storage_stream = self.subscribe(state_subscribeStorage).await?;
        let version_stream = self.subscribe(state_subscribeRuntimeVersion).await?;
        let head_stream = self.subscribe(chain_subscribeAllHeads).await?;

        Ok(SubscribedStream {
            storage_stream,
            version_stream,
            head_stream,
        })
    }

    async fn subscribe(
        &self,
        method: impl Into<String> + Send,
    ) -> Result<NotificationStream> {
        let res = Transport::send(&self.ws, method, None).await;

        match res {
            Err(err) => Err(err),
            Ok(v) => {
                let v = serde_json::to_string(&v).expect("encode back to string");
                let res: SubscribedSuccess = serde_json::from_str(&v)?;
                let id = match res.result {
                    SubscriptionId::String(s) => s,
                    SubscriptionId::Number(n) => n.to_string(),
                };
                let notifies = PubsubTransport::subscribe(&self.ws, id.into())?;
                Ok(notifies)
            }
        }
    }
}
