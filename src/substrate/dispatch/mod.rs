mod impls;
pub use self::impls::*;

use std::{collections::HashMap, fmt::Debug};

use async_jsonrpc_client::WsClientError;
use async_trait::async_trait;

use crate::{
    rpc_client::{NotificationStream, RpcClient},
    websocket::WsConnections,
};
use std::sync::Arc;

#[async_trait]
pub trait SubscriptionDispatcher: Send + Debug + 'static + Sync {
    fn method(&self) -> &'static str;

    // TODO: refine this
    async fn dispatch(&self, conns: WsConnections, stream: NotificationStream);
}

#[derive(Default)]
pub struct DispatcherHandler {
    dispatchers: HashMap<&'static str, Arc<dyn SubscriptionDispatcher>>,
}

impl DispatcherHandler {
    pub fn new() -> Self {
        Default::default()
    }

    /// register a subscription dispatcher
    pub fn register_dispatcher(&mut self, handle: impl SubscriptionDispatcher + 'static) {
        let method = handle.method();
        if self.dispatchers.insert(method, Arc::new(handle)).is_some() {
            panic!("`{}` dispatcher is duplicated", method);
        }
    }

    /// dispatch client's subscription data to every conns
    pub async fn start_dispatch(
        self,
        client: &RpcClient,
        conns: WsConnections,
    ) -> Result<(), WsClientError> {
        for (method, dispatcher) in self.dispatchers.into_iter() {
            let stream = client.subscribe(method, None).await?;
            dispatcher.dispatch(conns.clone(), stream).await;
        }
        Ok(())
    }
}
