//! Service related session handlers.
//! Send subscribed data to user according to subscription sessions

use std::sync::Arc;

use serde::{de::DeserializeOwned, Serialize};
use tokio::sync::RwLock;

use crate::{
    message::{
        serialize_subscribed_message, Id, SubscriptionNotification, SubscriptionNotificationParams,
    },
    session::{ISession, ISessions, Sessions},
    substrate::{
        constants,
        rpc::{
            chain::ChainHead,
            grandpa::GrandpaJustification,
            state::{RuntimeVersion, StateStorage},
        },
        session::{
            AllHeadSession, AllHeadSessions, FinalizedHeadSession, FinalizedHeadSessions,
            GrandpaJustificationSession, GrandpaJustificationSessions, NewHeadSession,
            NewHeadSessions, RuntimeVersionSession, RuntimeVersionSessions, StorageKeys,
            StorageSession, StorageSessions,
        },
    },
    websocket::WsConnection,
};

/// According to some states or sessions, we transform some data from chain node to new suitable values.
/// Mainly include some simple filtering operations.
pub trait SubscriptionTransformer {
    const METHOD: &'static str;
    type Input: Serialize + DeserializeOwned;
    type Session;

    fn transform(
        _session: &Self::Session,
        id: Id,
        input: Self::Input,
    ) -> Option<SubscriptionNotification<Self::Input>> {
        Some(SubscriptionNotification::new(
            Self::METHOD,
            SubscriptionNotificationParams {
                subscription: id,
                result: input,
            },
        ))
    }
}

pub struct ChainAllHeadTransformer;
impl SubscriptionTransformer for ChainAllHeadTransformer {
    const METHOD: &'static str = constants::chain_allHead;
    type Input = ChainHead;
    type Session = AllHeadSession;
}

pub struct ChainNewHeadTransformer;
impl SubscriptionTransformer for ChainNewHeadTransformer {
    const METHOD: &'static str = constants::chain_newHead;
    type Input = ChainHead;
    type Session = NewHeadSession;
}

pub struct ChainFinalizedHeadTransformer;
impl SubscriptionTransformer for ChainFinalizedHeadTransformer {
    const METHOD: &'static str = constants::chain_finalizedHead;
    type Input = ChainHead;
    type Session = FinalizedHeadSession;
}

pub struct StateRuntimeVersionTransformer;
impl SubscriptionTransformer for StateRuntimeVersionTransformer {
    const METHOD: &'static str = constants::state_runtimeVersion;
    type Input = RuntimeVersion;
    type Session = RuntimeVersionSession;
}

pub struct GrandpaJustificationTransformer;
impl SubscriptionTransformer for GrandpaJustificationTransformer {
    const METHOD: &'static str = constants::grandpa_justifications;
    type Input = GrandpaJustification;
    type Session = GrandpaJustificationSession;
}

pub struct StateStorageTransformer;
impl SubscriptionTransformer for StateStorageTransformer {
    const METHOD: &'static str = constants::state_storage;
    type Input = StateStorage;
    type Session = StorageSession;

    fn transform(
        session: &Self::Session,
        id: Id,
        input: Self::Input,
    ) -> Option<SubscriptionNotification<Self::Input>> {
        let (_, keys) = session;
        match keys {
            StorageKeys::All => Some(SubscriptionNotification::new(
                Self::METHOD,
                SubscriptionNotificationParams {
                    result: input,
                    subscription: id,
                },
            )),

            StorageKeys::Some(ref keys) => {
                let filtered_data = input
                    .changes
                    .clone()
                    .into_iter()
                    .filter(|(key, _)| keys.contains(key))
                    .collect::<Vec<(String, Option<String>)>>();

                if filtered_data.is_empty() {
                    None
                } else {
                    Some(SubscriptionNotification::new(
                        Self::METHOD,
                        SubscriptionNotificationParams {
                            result: StateStorage {
                                block: input.block,
                                changes: filtered_data,
                            },
                            subscription: id,
                        },
                    ))
                }
            }
        }
    }
}

// The followings are used to send subscription data to users

/// Send a subscription data according to Transformer's output
pub async fn send_subscription_data<ST, Session, Input>(
    sessions: Arc<RwLock<Sessions<Session>>>,
    conn: WsConnection,
    data: Input,
) where
    Session: 'static + ISession,
    Input: 'static + Serialize + Clone + Send,
    ST: SubscriptionTransformer<Session = Session, Input = Input>,
{
    for (subscription_id, session) in sessions.read().await.iter() {
        if let Some(data) = ST::transform(session, subscription_id.clone(), data.clone()) {
            let msg = serialize_subscribed_message(session, &data);
            let res = conn.send_compression_data(msg).await;
            // we need to cleanup unlived conn outside
            if let Err(err) = res {
                log::warn!(
                    "Error occurred when send {} data to peer `{}`: {:?}",
                    ST::METHOD,
                    conn.addr(),
                    err
                );
                return;
            }
        }
    }
}

pub fn send_state_storage(
    sessions: Arc<RwLock<StorageSessions>>,
    conn: WsConnection,
    data: StateStorage,
) {
    tokio::spawn(send_subscription_data::<StateStorageTransformer, _, _>(
        sessions, conn, data,
    ));
}

pub fn send_state_runtime_version(
    sessions: Arc<RwLock<RuntimeVersionSessions>>,
    conn: WsConnection,
    data: RuntimeVersion,
) {
    tokio::spawn(
        send_subscription_data::<StateRuntimeVersionTransformer, _, _>(sessions, conn, data),
    );
}

pub fn send_grandpa_justifications(
    sessions: Arc<RwLock<GrandpaJustificationSessions>>,
    conn: WsConnection,
    data: GrandpaJustification,
) {
    tokio::spawn(send_subscription_data::<
        GrandpaJustificationTransformer,
        _,
        _,
    >(sessions, conn, data));
}

pub fn send_chain_all_head(
    sessions: Arc<RwLock<AllHeadSessions>>,
    conn: WsConnection,
    data: ChainHead,
) {
    tokio::spawn(send_subscription_data::<ChainAllHeadTransformer, _, _>(
        sessions, conn, data,
    ));
}

pub fn send_chain_new_head(
    sessions: Arc<RwLock<NewHeadSessions>>,
    conn: WsConnection,
    data: ChainHead,
) {
    tokio::spawn(send_subscription_data::<ChainNewHeadTransformer, _, _>(
        sessions, conn, data,
    ));
}

pub fn send_chain_finalized_head(
    sessions: Arc<RwLock<FinalizedHeadSessions>>,
    conn: WsConnection,
    data: ChainHead,
) {
    tokio::spawn(
        send_subscription_data::<ChainFinalizedHeadTransformer, _, _>(sessions, conn, data),
    );
}
