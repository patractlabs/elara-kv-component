use std::{collections::HashSet, sync::Arc};

use tokio::sync::RwLock;

use crate::{
    session::{ISession, NoParamSession, NoParamSessions, Session, Sessions},
    Chain,
};

#[derive(Debug, Clone, Default)]
pub struct SubscriptionSessions {
    pub storage: Arc<RwLock<StorageSessions>>,
    pub runtime_version: Arc<RwLock<RuntimeVersionSessions>>,
    pub grandpa_justifications: Arc<RwLock<GrandpaJustificationSessions>>,
    pub all_head: Arc<RwLock<AllHeadSessions>>,
    pub new_head: Arc<RwLock<NewHeadSessions>>,
    pub finalized_head: Arc<RwLock<FinalizedHeadSessions>>,
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
    fn chain(&self) -> Chain {
        self.0.chain()
    }

    fn client_id(&self) -> String {
        self.0.client_id()
    }
}
pub type StorageSessions = Sessions<StorageSession>;

pub type WatchExtrinsicSession = (Session, String);

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
