use std::{
    collections::hash_map::{HashMap, Iter},
    fmt::Debug,
};

pub use jsonrpc_pubsub::manager::{IdProvider, NumericIdProvider, RandomStringIdProvider};

use crate::message::{RequestMessage, SubscriptionId};

pub trait ISession: Default + Clone + Send + Sync + Debug {
    fn chain_name(&self) -> String;

    fn client_id(&self) -> String;
}

/// Session as a subscription session
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct Session {
    // TODO: remove chain_name?
    pub chain_name: String,
    pub client_id: String,
}

impl ISession for Session {
    fn chain_name(&self) -> String {
        self.chain_name.clone()
    }

    fn client_id(&self) -> String {
        self.client_id.clone()
    }
}

impl From<&RequestMessage> for Session {
    fn from(msg: &RequestMessage) -> Self {
        Self {
            chain_name: msg.chain.clone(),
            client_id: msg.id.clone(),
        }
    }
}

/// when subscribed without any param, use this type to store sessions
pub type NoParamSession = Session;
/// when subscribed without any param, use this type to store sessions
pub type NoParamSessions = Sessions<NoParamSession>;

pub trait ISessions<S> {
    /// Returns the next ID for the subscription.
    fn new_subscription_id(&self) -> SubscriptionId;

    /// Returns a SubscriptionId for this storage.
    fn insert(&mut self, id: SubscriptionId, s: S) -> Option<S>;

    /// Removes a session from the sessions, returning the value at the session if the session
    /// was previously in the map.
    fn remove(&mut self, id: &SubscriptionId) -> Option<S>;

    /// An iterator visiting all key-value pairs in arbitrary order.
    fn iter(&self) -> Iter<'_, SubscriptionId, S>;

    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Sessions maintains the ws sessions for different subscriptions for one connection
#[derive(Default, Debug, Clone)]
pub struct Sessions<SessionItem, I: IdProvider = RandomStringIdProvider> {
    id_provider: I,
    map: HashMap<SubscriptionId, SessionItem>,
}

impl<S> ISessions<S> for Sessions<S> {
    /// Returns the next ID for the subscription.
    fn new_subscription_id(&self) -> SubscriptionId {
        let id = self.id_provider.next_id();
        id.into()
    }

    /// Returns a SubscriptionId for this storage.
    fn insert(&mut self, id: SubscriptionId, s: S) -> Option<S> {
        self.map.insert(id, s)
    }

    /// Removes a session from the sessions, returning the value at the session if the session
    /// was previously in the map.
    fn remove(&mut self, id: &SubscriptionId) -> Option<S> {
        self.map.remove(id)
    }

    /// An iterator visiting all key-value pairs in arbitrary order.
    fn iter(&self) -> Iter<'_, SubscriptionId, S> {
        self.map.iter()
    }

    fn len(&self) -> usize {
        self.map.len()
    }
}

impl<S> Sessions<S> {
    pub fn new() -> Self {
        Self {
            id_provider: Default::default(),
            map: Default::default(),
        }
    }
}

impl<S: Default, I: IdProvider> Sessions<S, I> {
    /// Creates a new SubscriptionManager with the specified
    /// ID provider.
    pub fn with_id_provider(id_provider: I) -> Self {
        Self {
            id_provider,
            map: Default::default(),
        }
    }
}
