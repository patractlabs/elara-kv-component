#[macro_use]
extern crate lazy_static;

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
mod util;

pub use error::ServiceError;

use jsonrpc_core::Version;
use jsonrpc_core::{Call, Id, Params, Success};
use web3::transports::WebSocket;

use web3::Result;
use web3::{DuplexTransport, Transport};

use crate::message::{SubscriptionId, Value};
use crate::websocket::WsConnection;
use futures::channel::mpsc::UnboundedReceiver;
use futures::Stream;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;

use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_stream::StreamExt;

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
    ws: WebSocket,
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
    storage_stream: UnboundedReceiver<Value>,
    version_stream: UnboundedReceiver<Value>,
    head_stream: UnboundedReceiver<Value>,
}

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
            let addr = *addr;
            let conn = conn.clone();
            // send data to conns
            sender(addr, conn, data.clone());
        }
    }
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
            |_addr, _conn, _data| {},
        ));
        // tokio::spawn(send_messages_to_conns(version_stream, conns.clone(), |_, _| {
        //     true
        // }));
        // tokio::spawn(send_messages_to_conns(head_stream, conns.clone()));
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
        let ws = web3::transports::WebSocket::new(addr.into().as_str()).await?;

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
        method: impl Into<String>,
    ) -> Result<UnboundedReceiver<Value>> {
        let res = Transport::execute(&self.ws, method.into().as_str(), vec![]).await;

        match res {
            Err(err) => Err(err),
            Ok(v) => {
                let v = serde_json::to_string(&v).expect("encode back to string");
                let res: SubscribedSuccess = serde_json::from_str(&v)?;
                let id = match res.result {
                    SubscriptionId::String(s) => s,
                    SubscriptionId::Number(n) => n.to_string(),
                };
                let notifies = DuplexTransport::subscribe(&self.ws, id.into())?;
                return Ok(notifies);
            }
        }
    }
}
