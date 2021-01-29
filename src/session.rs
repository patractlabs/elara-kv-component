use crate::message::{RequestMessage, SubscriptionId};
use std::collections::hash_map::Iter;
use std::collections::{HashMap, HashSet};

pub use jsonrpc_pubsub::manager::{
    IdProvider, NumericIdProvider, RandomStringIdProvider,
};

/// Sessions maintains the ws sessions for different subscriptions for one connection
#[derive(Default, Debug, Clone)]
pub struct Sessions<SessionItem, I: IdProvider = RandomStringIdProvider> {
    id_provider: I,
    map: HashMap<SubscriptionId, SessionItem>,
}

pub type StorageSession = (Session, StorageKeys<HashSet<String>>);
pub type StorageSessions = Sessions<StorageSession>;

// TODO: 为每种链定义各自的订阅的sessions

/// when subscribed without any param, use this type to store sessions
pub type NoParamSession = Session;
/// when subscribed without any param, use this type to store sessions
pub type NoParamSessions = Sessions<NoParamSession>;

// TODO: remove these
pub type ChainSession = (Session, SubscribedChainDataType);
pub type ChainSessions = Sessions<ChainSession>;

pub type Extrinsic = String;
pub type AuthorSession = (Session, Extrinsic);
pub type AuthorSessions = Sessions<AuthorSession>;

impl<S> Sessions<S> {
    pub fn new() -> Self {
        Self {
            id_provider: Default::default(),
            map: Default::default(),
        }
    }

    /// Returns the next ID for the subscription.
    pub fn new_subscription_id(&self) -> SubscriptionId {
        let id = self.id_provider.next_id();
        id.into()
    }

    /// Returns a SubscriptionId for this storage.
    pub fn insert(&mut self, id: SubscriptionId, s: S) -> Option<S> {
        self.map.insert(id, s)
    }

    /// Removes a session from the sessions, returning the value at the session if the session
    /// was previously in the map.
    pub fn remove(&mut self, id: &SubscriptionId) -> Option<S> {
        self.map.remove(id)
    }

    /// An iterator visiting all key-value pairs in arbitrary order.
    pub fn iter(&self) -> Iter<'_, SubscriptionId, S> {
        self.map.iter()
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

/// All represent for all storage keys. Some contains some keys.
#[derive(Clone, Debug)]
pub enum StorageKeys<T> {
    All,
    Some(T),
}

impl<T> Default for StorageKeys<T> {
    fn default() -> Self {
        Self::All
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubscribedChainDataType {
    AllHeads,
    NewHeads,
    FinalizedHeads,
    RuntimeVersion,
}

/// Session as a subscription session
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct Session {
    // TODO: we should maintains chain by typing them.
    pub chain_name: String,
    pub client_id: String,
}

impl From<&RequestMessage> for Session {
    fn from(msg: &RequestMessage) -> Self {
        Self {
            chain_name: msg.chain.clone(),
            client_id: msg.id.clone(),
        }
    }
}
