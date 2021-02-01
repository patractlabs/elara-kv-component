use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use crate::message::{
    Call, Id, Params, SubscribedSuccess, SubscriptionId, Success, Version,
};
pub use async_jsonrpc_client::RpcClientError;
use async_jsonrpc_client::WsTransport;
use async_jsonrpc_client::{NotificationStream, PubsubTransport, Transport};

pub type Result<T, E = RpcClientError> = std::result::Result<T, E>;

/// RpcClient is used to subscribe some data from different chain node.
pub struct RpcClient {
    ws: WsTransport,
}

// TODO: add config for different chain node
impl RpcClient {
    pub async fn new(addr: impl Into<String>) -> Result<Self> {
        let ws = WsTransport::new(addr.into().as_str()).await?;

        Ok(Self { ws })
    }

    /// subscribe a data from chain node, return a stream for it.
    pub async fn subscribe(
        &self,
        method: impl Into<String> + Send,
        params: Option<Params>,
    ) -> Result<NotificationStream> {
        let res = Transport::send(&self.ws, method, params).await;

        match res {
            Err(err) => Err(err),
            Ok(v) => {
                let v = serde_json::to_string(&v)
                    .expect("encode response message back to json string");
                // get the subscription id
                let res: SubscribedSuccess = serde_json::from_str(&v)?;
                let notifies = PubsubTransport::subscribe(&self.ws, res.result)?;
                Ok(notifies)
            }
        }
    }
}
