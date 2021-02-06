use crate::session::{ISession, NoParamSession, NoParamSessions, Session, Sessions};

use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Default)]
pub struct SubscriptionSessions {
    pub storage_sessions: Arc<RwLock<StorageSessions>>,
    pub runtime_version_sessions: Arc<RwLock<RuntimeVersionSessions>>,
    pub all_head_sessions: Arc<RwLock<AllHeadSessions>>,
    pub new_head_sessions: Arc<RwLock<NewHeadSessions>>,
    pub finalized_head_sessions: Arc<RwLock<FinalizedHeadSessions>>,
    pub watch_extrinsic_sessions: Arc<RwLock<WatchExtrinsicSessions>>,
}

pub type AllHeadSession = NoParamSession;
pub type AllHeadSessions = NoParamSessions;

pub type NewHeadSession = NoParamSession;
pub type NewHeadSessions = NoParamSessions;

pub type FinalizedHeadSession = NoParamSession;
pub type FinalizedHeadSessions = NoParamSessions;

pub type GrandpaJustificationSession = NoParamSession;
pub type GrandpaJustificationSessions = NoParamSessions;

pub type RuntimeVersionSession = NoParamSession;
pub type RuntimeVersionSessions = NoParamSessions;

pub type StorageSession = (Session, StorageKeys<HashSet<String>>);

impl ISession for StorageSession {
    fn chain_name(&self) -> String {
        self.0.chain_name()
    }

    fn client_id(&self) -> String {
        self.0.client_id()
    }
}
pub type StorageSessions = Sessions<StorageSession>;

pub type WatchExtrinsicSession = (Session, String);
pub type WatchExtrinsicSessions = Sessions<WatchExtrinsicSession>;

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
