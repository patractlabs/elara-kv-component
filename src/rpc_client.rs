use crate::message::{Id, Value};
use async_jsonrpc_client::{
    Output, Params, PubsubTransport, SubscriptionNotification, Transport, WsClient, WsClientError,
    WsSubscription,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

use crate::substrate::rpc_client::SubscriptionDispatcher;
use crate::websocket::WsConnections;
use crate::Chain;

pub type Result<T, E = WsClientError> = std::result::Result<T, E>;
pub type NotificationStream = WsSubscription<SubscriptionNotification>;

/// RpcClient is used to subscribe some data from different chain node.
#[derive(Clone)]
pub struct RpcClient {
    ws: WsClient,
    chain: Chain,
    addr: String,
}

pub type ArcRpcClient = Arc<RwLock<RpcClient>>;
pub type RpcClients = HashMap<Chain, ArcRpcClient>;

impl RpcClient {
    pub async fn new(chain: Chain, addr: impl Into<String>) -> Result<Self> {
        let addr = addr.into();
        let ws = WsClient::new(addr.as_str()).await?;

        Ok(Self { chain, ws, addr })
    }

    pub fn addr(&self) -> String {
        self.addr.clone()
    }

    pub fn node_name(&self) -> Chain {
        self.chain
    }

    #[inline]
    pub async fn system_health(&self) -> Result<Output> {
        self.ws.request("system_health", None).await
    }

    pub async fn state_get_storage(&self, key: String) -> Result<Output> {
        self.ws
            .request(
                "state_getStorage",
                Some(Params::Array(vec![Value::String(key)])),
            )
            .await
    }

    pub async fn is_alive(&self) -> bool {
        self.system_health().await.is_ok()
    }

    // After `reconnect`ed, client need to re`subscribe`.
    pub async fn reconnect(&mut self) -> Result<()> {
        self.ws = WsClient::new(self.addr.as_str()).await?;
        Ok(())
    }

    /// subscribe a data from chain node, return a stream for it.
    pub async fn subscribe<M>(
        &self,
        method: M,
        params: Option<Params>,
    ) -> Result<WsSubscription<SubscriptionNotification>, WsClientError>
    where
        M: Into<String> + Send,
    {
        let res = PubsubTransport::subscribe(&self.ws, method, params).await?;
        Ok(res.1)
    }

    pub async fn unsubscribe<M>(&self, method: M, id: Id) -> Result<bool, WsClientError>
    where
        M: Into<String> + Send,
    {
        let res = PubsubTransport::unsubscribe(&self.ws, method, id).await;
        res
    }
}
