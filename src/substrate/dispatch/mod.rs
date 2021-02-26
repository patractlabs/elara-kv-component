mod impls;
pub use self::impls::*;

use std::fmt::Debug;

use async_jsonrpc_client::WsClientError;

use crate::rpc_client::NotificationStream;
use crate::{rpc_client::RpcClient, websocket::WsConnections};
use std::collections::HashMap;

pub trait SubscriptionDispatcher: Send + Debug {
    fn method(&self) -> &'static str;

    // TODO: refine this
    fn dispatch(&mut self, conns: WsConnections, stream: NotificationStream);
}

pub struct DispatcherHandler {
    dispatchers: HashMap<&'static str, Box<dyn SubscriptionDispatcher>>,
}

impl DispatcherHandler {
    pub fn new() -> Self {
        Self {
            dispatchers: Default::default(),
        }
    }

    /// register a subscription dispatcher
    pub fn register_dispatcher(&mut self, handle: Box<dyn SubscriptionDispatcher>) {
        let method = handle.method();
        if self.dispatchers.insert(method, handle).is_some() {
            panic!("`{}` dispatcher is duplicated", method);
        }
    }

    /// dispatch client's subscription data to every conns
    pub async fn start_dispatch(
        &mut self,
        client: &RpcClient,
        conns: WsConnections,
    ) -> Result<(), WsClientError> {
        for (&method, dispatcher) in self.dispatchers.iter_mut() {
            let stream = client.subscribe(method, None).await?;
            let _res = dispatcher.dispatch(conns.clone(), stream);
        }

        Ok(())
    }
}
