//! Service related session handlers.
//! Send subscribed data to user according to subscription sessions
use crate::message::{
    serialize_elara_api, Id, SubscriptionNotification, SubscriptionNotificationParams,
    Version,
};
use crate::polkadot::consts;
use crate::polkadot::rpc_api::chain::ChainHead;
use crate::polkadot::rpc_api::grandpa::GrandpaJustification;
use crate::polkadot::rpc_api::state::{RuntimeVersion, StateStorage};
use crate::polkadot::session::{
    AllHeadSession, AllHeadSessions, FinalizedHeadSession, FinalizedHeadSessions,
    GrandpaJustificationSession, NewHeadSession, NewHeadSessions, RuntimeVersionSession,
    RuntimeVersionSessions, StorageKeys, StorageSession, StorageSessions,
};
use crate::session::{ISession, ISessions, Sessions};
use crate::websocket::WsConnection;
use log::*;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_tungstenite::tungstenite::error::Error;
use tokio_tungstenite::tungstenite::Message;
use crate::kusama::session::GrandpaJustificationSessions;

/// According to some states or sessions, we transform some data from chain node to new suitable values.
/// Mainly include some simple filtering operations.
pub trait SubscriptionTransformer {
    type Input: DeserializeOwned;
    type Output: Serialize;
    type Session;

    const METHOD: &'static str;

    fn transform(session: &Self::Session, id: Id, input: Self::Input) -> Self::Output;
}

pub struct StateStorageTransformer;

impl SubscriptionTransformer for StateStorageTransformer {
    type Input = StateStorage;
    type Output = SubscriptionNotification<StateStorage>;
    type Session = StorageSession;
    const METHOD: &'static str = consts::state_storage;

    fn transform(session: &Self::Session, id: Id, input: Self::Input) -> Self::Output {
        let (_, keys) = session;
        match keys {
            StorageKeys::All => {
                Self::Output {
                    jsonrpc: Version::V2_0,
                    method: Self::METHOD.to_string(),
                    params: SubscriptionNotificationParams {
                        result: input,
                        // we maintains the subscription id
                        subscription: id,
                    },
                }
            }

            StorageKeys::Some(ref keys) => {
                let filtered_data: Vec<(String, Option<String>)> = input
                    .changes
                    .clone()
                    .into_iter()
                    .filter(|(key, _)| keys.contains(key))
                    .collect();

                Self::Output {
                    jsonrpc: Version::V2_0,
                    method: consts::state_storage.to_string(),
                    params: SubscriptionNotificationParams {
                        result: StateStorage {
                            block: input.block,
                            changes: filtered_data,
                        },
                        // we maintains the subscription id
                        subscription: id,
                    },
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
    const METHOD: &'static str = consts::state_runtimeVersion;

    fn transform(_session: &Self::Session, id: Id, input: Self::Input) -> Self::Output {
        Self::Output {
            jsonrpc: Version::V2_0,
            method: Self::METHOD.to_string(),
            params: SubscriptionNotificationParams {
                result: input,
                // we maintains the subscription id
                subscription: id,
            },
        }
    }
}

pub struct GrandpaJustificationTransformer;

impl SubscriptionTransformer for GrandpaJustificationTransformer {
    type Input = GrandpaJustification;
    type Output = SubscriptionNotification<GrandpaJustification>;
    type Session = GrandpaJustificationSession;
    const METHOD: &'static str = consts::grandpa_justifications;

    fn transform(_session: &Self::Session, id: Id, input: Self::Input) -> Self::Output {
        Self::Output {
            jsonrpc: Version::V2_0,
            method: Self::METHOD.to_string(),
            params: SubscriptionNotificationParams {
                result: input,
                // we maintains the subscription id
                subscription: id,
            },
        }
    }
}

pub struct ChainAllHeadTransformer;

impl SubscriptionTransformer for ChainAllHeadTransformer {
    type Input = ChainHead;
    type Output = SubscriptionNotification<ChainHead>;
    type Session = AllHeadSession;
    const METHOD: &'static str = consts::chain_allHead;

    fn transform(_session: &Self::Session, id: Id, input: Self::Input) -> Self::Output {
        Self::Output {
            jsonrpc: Version::V2_0,
            method: Self::METHOD.to_string(),
            params: SubscriptionNotificationParams {
                result: input,
                // we maintains the subscription id
                subscription: id,
            },
        }
    }
}

pub struct ChainNewHeadTransformer;

impl SubscriptionTransformer for ChainNewHeadTransformer {
    type Input = ChainHead;
    type Output = SubscriptionNotification<ChainHead>;
    type Session = NewHeadSession;
    const METHOD: &'static str = consts::chain_newHead;

    fn transform(_session: &Self::Session, id: Id, input: Self::Input) -> Self::Output {
        Self::Output {
            jsonrpc: Version::V2_0,
            method: Self::METHOD.to_string(),
            params: SubscriptionNotificationParams {
                result: input,
                // we maintains the subscription id
                subscription: id,
            },
        }
    }
}

pub struct ChainFinalizedHeadTransformer;

impl SubscriptionTransformer for ChainFinalizedHeadTransformer {
    type Input = ChainHead;
    type Output = SubscriptionNotification<ChainHead>;
    type Session = FinalizedHeadSession;
    const METHOD: &'static str = consts::chain_finalizedHead;

    fn transform(_session: &Self::Session, id: Id, input: Self::Input) -> Self::Output {
        Self::Output {
            jsonrpc: Version::V2_0,
            method: Self::METHOD.to_string(),
            params: SubscriptionNotificationParams {
                result: input,
                // we maintains the subscription id
                subscription: id,
            },
        }
    }
}

async fn send_message(conn: WsConnection, msg: String, data_type: &'static str) {
    let res = conn.send_message(Message::Text(msg)).await;
    // we need to cleanup unlived conn outside
    if let Err(err) = res {
        if let Error::ConnectionClosed = err {
            conn.close();
        }

        warn!(
            "Error occurred when send {} data to peer `{}`: {:?}",
            data_type,
            conn.addr(),
            err
        );
    };
}

// The followings are used to send subscription data to users

async fn send_subscription_data<ST, Session, Input>(
    sessions: Arc<RwLock<Sessions<Session>>>,
    conn: WsConnection,
    data: Input,
) where
    Session: 'static + ISession,
    Input: 'static + Clone + Send,
    ST: SubscriptionTransformer<Session = Session, Input = Input>,
{
    for (subscription_id, session) in sessions.read().await.iter() {
        let data = ST::transform(session, subscription_id.clone().into(), data.clone());
        // two level json
        let msg = serialize_elara_api(session, &data);
        send_message(conn.clone(), msg, ST::METHOD).await;
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
        send_subscription_data::<StateRuntimeVersionTransformer, _, _>(
            sessions, conn, data,
        ),
    );
}

pub fn send_grandpa_justifications(
    sessions: Arc<RwLock<GrandpaJustificationSessions>>,
    conn: WsConnection,
    data: GrandpaJustification,
) {
    tokio::spawn(
        send_subscription_data::<GrandpaJustificationTransformer, _, _>(
            sessions, conn, data,
        ),
    );
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
        send_subscription_data::<ChainFinalizedHeadTransformer, _, _>(
            sessions, conn, data,
        ),
    );
}
