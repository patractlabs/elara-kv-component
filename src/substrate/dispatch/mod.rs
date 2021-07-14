mod impls;
pub use self::impls::*;

use std::{collections::HashMap, fmt::Debug};

use async_jsonrpc_client::WsClientError;
use async_trait::async_trait;

use crate::{
    rpc_client::{NotificationStream, RpcClient},
    websocket::WsConnections,
};

/// SubscriptionDispatcher handle a subscription data stream according to method.
#[async_trait]
pub trait SubscriptionDispatcher: Send + Debug + 'static + Sync {
    /// Subscription method.
    fn method(&self) -> &'static str;

    /// Dispatch subscription task. It should be called only once.
    async fn dispatch(&self, conns: WsConnections, stream: NotificationStream);
}

#[derive(Clone, Debug)]
pub enum DispatcherType {
    StateStorageDispatcher(StateStorageDispatcher),
    StateRuntimeVersionDispatcher(StateRuntimeVersionDispatcher),
    ChainNewHeadDispatcher(ChainNewHeadDispatcher),
    ChainFinalizedHeadDispatcher(ChainFinalizedHeadDispatcher),
    ChainAllHeadDispatcher(ChainAllHeadDispatcher),
    GrandpaJustificationDispatcher(GrandpaJustificationDispatcher),
}

impl From<StateStorageDispatcher> for DispatcherType {
    fn from(s: StateStorageDispatcher) -> Self {
        DispatcherType::StateStorageDispatcher(s)
    }
}

impl From<StateRuntimeVersionDispatcher> for DispatcherType {
    fn from(s: StateRuntimeVersionDispatcher) -> Self {
        DispatcherType::StateRuntimeVersionDispatcher(s)
    }
}

impl From<ChainNewHeadDispatcher> for DispatcherType {
    fn from(s: ChainNewHeadDispatcher) -> Self {
        DispatcherType::ChainNewHeadDispatcher(s)
    }
}

impl From<ChainFinalizedHeadDispatcher> for DispatcherType {
    fn from(s: ChainFinalizedHeadDispatcher) -> Self {
        DispatcherType::ChainFinalizedHeadDispatcher(s)
    }
}

impl From<ChainAllHeadDispatcher> for DispatcherType {
    fn from(s: ChainAllHeadDispatcher) -> Self {
        DispatcherType::ChainAllHeadDispatcher(s)
    }
}

impl From<GrandpaJustificationDispatcher> for DispatcherType {
    fn from(s: GrandpaJustificationDispatcher) -> Self {
        DispatcherType::GrandpaJustificationDispatcher(s)
    }
}

impl DispatcherType {
    async fn dispatch(&self, conns: WsConnections, stream: NotificationStream) {
        use DispatcherType::*;
        match self {
            StateStorageDispatcher(s) => s.dispatch(conns, stream).await,
            StateRuntimeVersionDispatcher(s) => s.dispatch(conns, stream).await,
            ChainNewHeadDispatcher(s) => s.dispatch(conns, stream).await,
            ChainFinalizedHeadDispatcher(s) => s.dispatch(conns, stream).await,
            ChainAllHeadDispatcher(s) => s.dispatch(conns, stream).await,
            GrandpaJustificationDispatcher(s) => s.dispatch(conns, stream).await,
        };
    }
}

/// DispatcherHandler manage lifetime of all dispatchers.
#[derive(Default, Clone)]
pub struct DispatcherHandler {
    dispatchers: HashMap<&'static str, DispatcherType>,
}

impl DispatcherHandler {
    pub fn new() -> Self {
        Default::default()
    }

    /// Returns the inner all dispatchers.
    pub fn dispatchers(&self) -> &HashMap<&'static str, DispatcherType> {
        &self.dispatchers
    }

    /// Register a subscription dispatcher.
    pub fn register_dispatcher<T: SubscriptionDispatcher>(&mut self, dispatcher: T)
    where
        DispatcherType: From<T>,
    {
        let method = dispatcher.method();
        if self
            .dispatchers
            .insert(method, DispatcherType::from(dispatcher))
            .is_some()
        {
            panic!("`{}` dispatcher is duplicated", method);
        }
    }

    /// Dispatch client's subscription data to every conns.
    pub async fn start_dispatch(
        &self,
        client: &RpcClient,
        conns: WsConnections,
    ) -> Result<(), WsClientError> {
        for (&method, dispatcher) in self.dispatchers.iter() {
            let stream = client.subscribe(method, None).await?;
            dispatcher.dispatch(conns.clone(), stream).await;
        }
        Ok(())
    }
}
