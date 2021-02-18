use crate::message::Params;
use async_jsonrpc_client::PubsubTransport;
pub use async_jsonrpc_client::WsClientError;
use async_jsonrpc_client::{SubscriptionNotification, WsClient, WsSubscription};

pub type Result<T, E = WsClientError> = std::result::Result<T, E>;

/// RpcClient is used to subscribe some data from different chain node.
pub struct RpcClient {
    ws: WsClient,
    node: String,
    addr: String,
}

impl RpcClient {
    pub async fn new(node: String, addr: impl Into<String>) -> Result<Self> {
        let addr = addr.into();
        let ws = WsClient::new(addr.as_str()).await?;

        Ok(Self { node, ws, addr })
    }

    pub fn addr(&self) -> String {
        self.addr.clone()
    }

    pub fn node_name(&self) -> String {
        self.node.clone()
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
