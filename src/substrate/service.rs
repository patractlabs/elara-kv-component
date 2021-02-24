//! Service related session handlers.
//! Send subscribed data to user according to subscription sessions

use std::sync::Arc;

use serde::{de::DeserializeOwned, Serialize};
use tokio::sync::RwLock;
use tokio_tungstenite::tungstenite::Message;

use crate::{
    message::{
        serialize_subscribed_message, Id, SubscriptionNotification, SubscriptionNotificationParams,
        Version,
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
    type Input: DeserializeOwned;
    type Output: Serialize;
    type Session;

    const METHOD: &'static str;

    fn transform(session: &Self::Session, id: Id, input: Self::Input) -> Option<Self::Output>;
}

pub struct StateStorageTransformer;

impl SubscriptionTransformer for StateStorageTransformer {
    type Input = StateStorage;
    type Output = SubscriptionNotification<StateStorage>;
    type Session = StorageSession;
    const METHOD: &'static str = constants::state_storage;

    fn transform(session: &Self::Session, id: Id, input: Self::Input) -> Option<Self::Output> {
        let (_, keys) = session;
        match keys {
            StorageKeys::All => {
                Some(Self::Output {
                    jsonrpc: Version::V2_0,
                    method: Self::METHOD.to_string(),
                    params: SubscriptionNotificationParams {
                        result: input,
                        // we maintains the subscription id
                        subscription: id,
                    },
                })
            }

            StorageKeys::Some(ref keys) => {
                let filtered_data: Vec<(String, Option<String>)> = input
                    .changes
                    .clone()
                    .into_iter()
                    .filter(|(key, _)| keys.contains(key))
                    .collect();

                if filtered_data.is_empty() {
                    None
                } else {
                    Some(Self::Output {
                        jsonrpc: Version::V2_0,
                        method: constants::state_storage.to_string(),
                        params: SubscriptionNotificationParams {
                            result: StateStorage {
                                block: input.block,
                                changes: filtered_data,
                            },
                            // we maintains the subscription id
                            subscription: id,
                        },
                    })
                }
            }
        }
    }
}

pub struct StateRuntimeVersionTransformer;

impl SubscriptionTransformer for StateRuntimeVersionTransformer {
    type Input = RuntimeVersion;
    type Output = SubscriptionNotification<RuntimeVersion>;
    type Session = RuntimeVersionSession;
    const METHOD: &'static str = constants::state_runtimeVersion;

    fn transform(_session: &Self::Session, id: Id, input: Self::Input) -> Option<Self::Output> {
        Some(Self::Output {
            jsonrpc: Version::V2_0,
            method: Self::METHOD.to_string(),
            params: SubscriptionNotificationParams {
                result: input,
                // we maintains the subscription id
                subscription: id,
            },
        })
    }
}

pub struct GrandpaJustificationTransformer;

impl SubscriptionTransformer for GrandpaJustificationTransformer {
    type Input = GrandpaJustification;
    type Output = SubscriptionNotification<GrandpaJustification>;
    type Session = GrandpaJustificationSession;
    const METHOD: &'static str = constants::grandpa_justifications;

    fn transform(_session: &Self::Session, id: Id, input: Self::Input) -> Option<Self::Output> {
        Some(Self::Output {
            jsonrpc: Version::V2_0,
            method: Self::METHOD.to_string(),
            params: SubscriptionNotificationParams {
                result: input,
                // we maintains the subscription id
                subscription: id,
            },
        })
    }
}

pub struct ChainAllHeadTransformer;

impl SubscriptionTransformer for ChainAllHeadTransformer {
    type Input = ChainHead;
    type Output = SubscriptionNotification<ChainHead>;
    type Session = AllHeadSession;
    const METHOD: &'static str = constants::chain_allHead;

    fn transform(_session: &Self::Session, id: Id, input: Self::Input) -> Option<Self::Output> {
        Some(Self::Output {
            jsonrpc: Version::V2_0,
            method: Self::METHOD.to_string(),
            params: SubscriptionNotificationParams {
                result: input,
                // we maintains the subscription id
                subscription: id,
            },
        })
    }
}

pub struct ChainNewHeadTransformer;

impl SubscriptionTransformer for ChainNewHeadTransformer {
    type Input = ChainHead;
    type Output = SubscriptionNotification<ChainHead>;
    type Session = NewHeadSession;
    const METHOD: &'static str = constants::chain_newHead;

    fn transform(_session: &Self::Session, id: Id, input: Self::Input) -> Option<Self::Output> {
        Some(Self::Output {
            jsonrpc: Version::V2_0,
            method: Self::METHOD.to_string(),
            params: SubscriptionNotificationParams {
                result: input,
                // we maintains the subscription id
                subscription: id,
            },
        })
    }
}

pub struct ChainFinalizedHeadTransformer;

impl SubscriptionTransformer for ChainFinalizedHeadTransformer {
    type Input = ChainHead;
    type Output = SubscriptionNotification<ChainHead>;
    type Session = FinalizedHeadSession;
    const METHOD: &'static str = constants::chain_finalizedHead;

    fn transform(_session: &Self::Session, id: Id, input: Self::Input) -> Option<Self::Output> {
        Some(Self::Output {
            jsonrpc: Version::V2_0,
            method: Self::METHOD.to_string(),
            params: SubscriptionNotificationParams {
                result: input,
                // we maintains the subscription id
                subscription: id,
            },
        })
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
    Input: 'static + Clone + Send,
    ST: SubscriptionTransformer<Session = Session, Input = Input>,
{
    for (subscription_id, session) in sessions.read().await.iter() {
        let data = ST::transform(session, subscription_id.clone(), data.clone());
        match data {
            None => {}
            Some(data) => {
                // two level json
                let msg = serialize_subscribed_message(session, &data);
                let res = conn.send_message(Message::Text(msg)).await;
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
