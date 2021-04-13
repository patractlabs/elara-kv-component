use std::{collections::HashMap, sync::Arc};

use async_jsonrpc_client::{
    Output, Params, PubsubTransport, SubscriptionNotification, Transport, WsClient, WsClientError,
    WsSubscription,
};
use tokio::sync::RwLock;

use crate::{
    config::RpcClientConfig,
    message::Id,
    substrate::constants::{state_getRuntimeVersion, system_health},
    Chain,
};

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
    pub async fn new(
        chain: Chain,
        addr: impl Into<String>,
        config: Option<RpcClientConfig>,
    ) -> Result<Self> {
        let addr = addr.into();
        let ws = if let Some(config) = config {
            WsClient::builder()
                .max_concurrent_request_capacity(config.max_request_cap.unwrap_or(256))
                .max_capacity_per_subscription(config.max_cap_per_subscription.unwrap_or(64))
                .build(addr.as_str())
                .await?
        } else {
            WsClient::new(addr.as_str()).await?
        };

        Ok(Self { chain, ws, addr })
    }

    #[inline]
    pub fn addr(&self) -> String {
        self.addr.clone()
    }

    #[inline]
    pub fn chain(&self) -> Chain {
        self.chain.clone()
    }

    #[inline]
    pub async fn system_health(&self) -> Result<Output> {
        self.ws.request(system_health, None).await
    }

    pub async fn get_runtime_version(&self) -> Result<Output> {
        self.ws.request(state_getRuntimeVersion, None).await
    }

    #[inline]
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
