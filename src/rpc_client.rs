use crate::message::{Params, SubscribedSuccess};
pub use async_jsonrpc_client::RpcClientError;
use async_jsonrpc_client::WsTransport;
use async_jsonrpc_client::{NotificationStream, PubsubTransport, Transport};

pub type Result<T, E = RpcClientError> = std::result::Result<T, E>;

/// RpcClient is used to subscribe some data from different chain node.
pub struct RpcClient {
    ws: WsTransport,
    node: String,
    addr: String,
}

impl RpcClient {
    pub async fn new(node: String, addr: impl Into<String>) -> Result<Self> {
        let addr = addr.into();
        let ws = WsTransport::new(addr.as_str()).await?;

        Ok(Self { node, ws, addr })
    }

    pub fn addr(&self) -> String {
        self.addr.clone()
    }

    pub fn node_name(&self) -> String {
        self.node.clone()
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
