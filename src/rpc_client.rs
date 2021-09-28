use std::{collections::HashMap, sync::Arc};

use async_jsonrpc_client::{
    Output, Params, PubsubTransport, SubscriptionNotification, Transport, WsClient, WsClientError,
    WsSubscription,
};
use tokio::sync::RwLock;

use crate::message::Value;
use crate::substrate::constants::state_queryStorageAt;
use crate::substrate::dispatch::{DispatcherHandler, DispatcherType};
use crate::{
    config::RpcClientConfig,
    message::Id,
    substrate::constants::{state_getRuntimeVersion, system_health},
    Chain,
};
use tokio::time::Duration;

pub type Result<T, E = WsClientError> = std::result::Result<T, E>;
pub type NotificationStream = WsSubscription<SubscriptionNotification>;

/// RpcClient is used to subscribe some data from different chain node.
#[derive(Clone)]
pub struct RpcClient {
    ws: WsClient,
    chain: Chain,
    addr: String,
    config: Option<RpcClientConfig>,
    // TODO: use arc for splitting lifetime
    pub ctx: Option<RpcClientCtx>,
}

/// Context for a rpc client.
/// It may contains some caches for subscription data.
#[derive(Clone)]
pub struct RpcClientCtx {
    pub handler: DispatcherHandler,
}

pub type ArcRpcClient = Arc<RwLock<RpcClient>>;
pub type RpcClients = HashMap<Chain, ArcRpcClient>;

impl RpcClient {
    async fn create_ws_client(addr: &str, config: Option<RpcClientConfig>) -> Result<WsClient> {
        let ws = if let Some(config) = config {
            WsClient::builder()
                .timeout(Duration::from_millis(config.timeout_ms.unwrap_or(3000)))
                .max_concurrent_request_capacity(config.max_request_cap.unwrap_or(256))
                .max_capacity_per_subscription(config.max_cap_per_subscription.unwrap_or(64))
                .build(addr)
                .await?
        } else {
            WsClient::new(addr).await?
        };
        Ok(ws)
    }

    pub async fn new(
        chain: Chain,
        addr: impl Into<String>,
        config: Option<RpcClientConfig>,
    ) -> Result<Self> {
        let addr = addr.into();
        let ws = Self::create_ws_client(addr.as_str(), config).await?;

        Ok(Self {
            chain,
            ws,
            addr,
            config,
            ctx: Default::default(),
        })
    }

    #[inline]
    pub fn addr(&self) -> String {
        self.addr.clone()
    }

    pub fn dispatcher_ref(&self, method: &str) -> Option<&DispatcherType> {
        let ctx = self.ctx.as_ref().expect("get client context");
        ctx.handler.dispatchers().get(method)
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

    pub async fn query_storage_at(&self, keys: Vec<Value>) -> Result<Output> {
        self.ws
            .request(
                state_queryStorageAt,
                Some(Params::Array(vec![Value::Array(keys)])),
            )
            .await
    }

    #[inline]
    pub async fn is_alive(&self) -> bool {
        let output = self.system_health().await;
        match output {
            Err(err) => {
                log::info!("system_health failed for '{}': {:?}", self.chain, err);
                false
            }
            Ok(output) => {
                log::info!("system_health check for '{}': {}", self.chain, output);
                true
            }
        }
    }

    // After `reconnect`ed, client need to re`subscribe`.
    pub async fn reconnect(&mut self) -> Result<()> {
        self.ws = Self::create_ws_client(self.addr.as_str(), self.config).await?;
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
