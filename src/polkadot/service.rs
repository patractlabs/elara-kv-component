//! Service related session handlers.
//! Send subscribed data to user according to subscription sessions

use jsonrpc_types::{SubscriptionNotification, SubscriptionNotificationParams};
use log::*;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::net::SocketAddr;
use tokio_tungstenite::tungstenite::Message;

use crate::error::ServiceError;
use crate::message::{serialize_elara_api, Id, Version};
use crate::polkadot::client::MethodReceivers;
use crate::polkadot::consts;
use crate::polkadot::rpc_api::chain::ChainHead;
use crate::polkadot::rpc_api::state::{RuntimeVersion, StateStorage};
use crate::polkadot::session::{
    AllHeadSession, FinalizedHeadSession, NewHeadSession, RuntimeVersionSession,
    StorageKeys, StorageSession,
};
use crate::session::{ISessions, NoParamSession, Session};
use crate::websocket::WsConnection;

// TODO: refine these api
/// according to some states or sessions, we transform some data from chain node to new suitable values.
/// Mainly include some simple filtering operations.
pub trait SubscriptionTransformer<'a> {
    type Input: DeserializeOwned;
    type Output: Serialize;

    const METHOD: &'static str;

    fn transform(&'a self, input: Self::Input) -> Self::Output;
}

pub struct StateStorageTransformer<'a> {
    session: &'a StorageSession,
    subscription_id: Id,
}

pub struct StateRuntimeVersionTransformer<'a> {
    session: &'a RuntimeVersionSession,
    subscription_id: Id,
}

pub struct ChainAllHeadTransformer<'a> {
    session: &'a AllHeadSession,
    subscription_id: Id,
}

pub struct ChainNewHeadTransformer<'a> {
    session: &'a NewHeadSession,
    subscription_id: Id,
}

pub struct ChainFinalizedHeadTransformer<'a> {
    session: &'a FinalizedHeadSession,
    subscription_id: Id,
}

impl<'a> SubscriptionTransformer<'a> for ChainAllHeadTransformer<'a> {
    type Input = ChainHead;
    type Output = SubscriptionNotification<ChainHead>;
    const METHOD: &'static str = consts::chain_allHead;

    // TODO: use macro to impl these no param transform
    fn transform(&'a self, input: Self::Input) -> Self::Output {
        Self::Output {
            jsonrpc: Version::V2_0,
            method: Self::METHOD.to_string(),
            params: SubscriptionNotificationParams {
                result: input,
                // we maintains the subscription id
                subscription: self.subscription_id.clone(),
            },
        }
    }
}

impl<'a> SubscriptionTransformer<'a> for ChainNewHeadTransformer<'a> {
    type Input = ChainHead;
    type Output = SubscriptionNotification<ChainHead>;
    const METHOD: &'static str = consts::chain_newHead;

    // TODO: use macro to impl these no param transform
    fn transform(&'a self, input: Self::Input) -> Self::Output {
        Self::Output {
            jsonrpc: Version::V2_0,
            method: Self::METHOD.to_string(),
            params: SubscriptionNotificationParams {
                result: input,
                // we maintains the subscription id
                subscription: self.subscription_id.clone(),
            },
        }
    }
}

impl<'a> SubscriptionTransformer<'a> for ChainFinalizedHeadTransformer<'a> {
    type Input = ChainHead;
    type Output = SubscriptionNotification<ChainHead>;
    const METHOD: &'static str = consts::chain_finalizedHead;

    // TODO: use macro to impl these no param transform
    fn transform(&'a self, input: Self::Input) -> Self::Output {
        Self::Output {
            jsonrpc: Version::V2_0,
            method: Self::METHOD.to_string(),
            params: SubscriptionNotificationParams {
                result: input,
                // we maintains the subscription id
                subscription: self.subscription_id.clone(),
            },
        }
    }
}

impl<'a> SubscriptionTransformer<'a> for StateRuntimeVersionTransformer<'a> {
    type Input = RuntimeVersion;
    type Output = SubscriptionNotification<RuntimeVersion>;
    const METHOD: &'static str = consts::state_runtimeVersion;

    fn transform(&'a self, input: Self::Input) -> Self::Output {
        Self::Output {
            jsonrpc: Version::V2_0,
            method: Self::METHOD.to_string(),
            params: SubscriptionNotificationParams {
                result: input,
                // we maintains the subscription id
                subscription: self.subscription_id.clone(),
            },
        }
    }
}

impl<'a> SubscriptionTransformer<'a> for StateStorageTransformer<'a> {
    type Input = StateStorage;
    type Output = SubscriptionNotification<StateStorage>;
    const METHOD: &'static str = consts::state_storage;

    fn transform(&'a self, input: Self::Input) -> Self::Output {
        let (_, keys) = self.session;
        match keys {
            StorageKeys::All => {
                Self::Output {
                    jsonrpc: Version::V2_0,
                    method: Self::METHOD.to_string(),
                    params: SubscriptionNotificationParams {
                        result: input.clone(),
                        // we maintains the subscription id
                        subscription: self.subscription_id.clone(),
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
                            block: input.block.clone(),
                            changes: filtered_data,
                        },
                        // we maintains the subscription id
                        subscription: self.subscription_id.clone().into(),
                    },
                }
            }
        }
    }
}

use tokio_tungstenite::tungstenite;

fn send_message(
    addr: SocketAddr,
    conn: WsConnection,
    msg: String,
    data_type: &'static str,
) {
    tokio::spawn(async move {
        let res = conn.send_message(Message::Text(msg)).await;
        // match res {
        //     Err(ServiceError::WsServerError(tungstenite::Error::ConnectionClosed)) => {
        //         // TODO: remove the connection
        //         // conn
        //     }
        // }
        if let Err(err) = res {
            warn!(
                "Error occurred when send {} data to peer `{}`: {:?}",
                data_type, addr, err
            );
        }
    });
}

// The followings are used to send data to users

// TODO: we should split session and connection
pub fn send_state_storage(addr: SocketAddr, conn: WsConnection, data: StateStorage) {
    tokio::spawn(async move {
        for (subscription_id, session) in
            conn.polkadot_sessions.storage_sessions.read().await.iter()
        {
            let transformer = StateStorageTransformer {
                session,
                subscription_id: subscription_id.clone().into(),
            };

            let data = transformer.transform(data.clone());
            let (session, _) = session;
            // two level json
            let msg = serialize_elara_api(session, &data);
            send_message(addr, conn.clone(), msg, StateStorageTransformer::METHOD)
        }
    });
}

pub fn send_state_runtime_version(
    addr: SocketAddr,
    conn: WsConnection,
    data: RuntimeVersion,
) {
    tokio::spawn(async move {
        for (subscription_id, session) in conn
            .polkadot_sessions
            .runtime_version_sessions
            .read()
            .await
            .iter()
        {
            let transformer = StateRuntimeVersionTransformer {
                session,
                subscription_id: subscription_id.clone().into(),
            };

            let data = transformer.transform(data.clone());
            // two level json
            let msg = serialize_elara_api(session, &data);
            send_message(
                addr,
                conn.clone(),
                msg,
                StateRuntimeVersionTransformer::METHOD,
            )
        }
    });
}

pub fn send_chain_all_head(addr: SocketAddr, conn: WsConnection, data: ChainHead) {
    tokio::spawn(async move {
        for (subscription_id, session) in
            conn.polkadot_sessions.all_head_sessions.read().await.iter()
        {
            let transformer = ChainAllHeadTransformer {
                session,
                subscription_id: subscription_id.clone().into(),
            };
            let data = transformer.transform(data.clone());
            // two level json
            let msg = serialize_elara_api(session, &data);
            send_message(addr, conn.clone(), msg, ChainAllHeadTransformer::METHOD)
        }
    });
}

pub fn send_chain_new_head(addr: SocketAddr, conn: WsConnection, data: ChainHead) {
    tokio::spawn(async move {
        for (subscription_id, session) in
            conn.polkadot_sessions.new_head_sessions.read().await.iter()
        {
            let transformer = ChainNewHeadTransformer {
                session,
                subscription_id: subscription_id.clone().into(),
            };
            let data = transformer.transform(data.clone());
            // two level json
            let msg = serialize_elara_api(session, &data);
            send_message(addr, conn.clone(), msg, ChainNewHeadTransformer::METHOD)
        }
    });
}

pub fn send_chain_finalized_head(addr: SocketAddr, conn: WsConnection, data: ChainHead) {
    tokio::spawn(async move {
        for (subscription_id, session) in
            conn.polkadot_sessions.new_head_sessions.read().await.iter()
        {
            let transformer = ChainFinalizedHeadTransformer {
                session,
                subscription_id: subscription_id.clone().into(),
            };
            let data = transformer.transform(data.clone());
            // two level json
            let msg = serialize_elara_api(session, &data);
            send_message(
                addr,
                conn.clone(),
                msg,
                ChainFinalizedHeadTransformer::METHOD,
            )
        }
    });
}
