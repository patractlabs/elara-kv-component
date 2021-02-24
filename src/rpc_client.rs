use async_jsonrpc_client::{
    Output, Params, PubsubTransport, SubscriptionNotification, Transport, WsClient, WsClientError,
    WsSubscription,
};

use crate::Chain;

pub type Result<T, E = WsClientError> = std::result::Result<T, E>;
pub type NotificationStream = WsSubscription<SubscriptionNotification>;

/// RpcClient is used to subscribe some data from different chain node.
pub struct RpcClient {
    ws: WsClient,
    node: Chain,
    addr: String,
}

impl RpcClient {
    pub async fn new(node: Chain, addr: impl Into<String>) -> Result<Self> {
        let addr = addr.into();
        let ws = WsClient::new(addr.as_str()).await?;

        Ok(Self { node, ws, addr })
    }

    pub fn addr(&self) -> String {
        self.addr.clone()
    }

    pub fn node_name(&self) -> Chain {
        self.node
    }

    #[inline]
    pub async fn system_health(&self) -> Result<Output> {
        self.ws.request("system_health", None).await
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
        subscribe_method: M,
        params: Option<Params>,
    ) -> Result<WsSubscription<SubscriptionNotification>, WsClientError>
    where
        M: Into<String> + Send,
    {
        let res = PubsubTransport::subscribe(&self.ws, subscribe_method, params).await?;
        Ok(res.1)
    }
}
