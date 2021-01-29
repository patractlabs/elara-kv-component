//! Service related session handlers.
//! Send subscribed data to user according to subscription sessions

use futures::Stream;
use jsonrpc_types::{SubscriptionNotification, SubscriptionNotificationParams};
use log::*;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio_tungstenite::tungstenite::Message;

use crate::message::{
    serialize_elara_api, Id, SubscribedData, SubscribedMessage, SubscribedParams, Version,
};
use crate::polkadot::client::MethodReceivers;
use crate::polkadot::consts;
use crate::polkadot::rpc_api::chain::ChainHead;
use crate::polkadot::rpc_api::state::{RuntimeVersion, StateStorage};
use crate::polkadot::rpc_api::SubscribedResult;
use crate::polkadot::session::{RuntimeVersionSession, RuntimeVersionSessions, StorageKeys, StorageSession, StorageSessions, AllHeadSession};
use crate::session::{ISessions, Session, NoParamSession};
use crate::websocket::WsConnection;

// 根据elara订阅的会话，把从节点订阅到的数据转换成用户实际接收到的数据
// 一般是做数据的简单过滤操作，和修改一些状态/会话相关的信息，最后序列化成jsonrpc封装到elara api的字段里

// TODD:
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
            let conn = conn.clone();
            tokio::spawn(async move {
                let res = conn.send_message(Message::Text(msg)).await;
                res.map_err(|err| {
                    warn!(
                        "Error occurred when send StateStorageResult data to `{}`: {:?}",
                        addr, err
                    );
                });
            });
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
            let conn = conn.clone();
            tokio::spawn(async move {
                let res = conn.send_message(Message::Text(msg)).await;
                if let Err(err) = res {
                    warn!(
                        "Error occurred when send RuntimeVersion data to `{}`: {:?}",
                        addr, err
                    );
                }
            });
        }
    });
}

// TODO: impl new head and finalized head
pub fn send_chain_all_head(addr: SocketAddr, conn: WsConnection, data: ChainHead) {
    tokio::spawn(async move {
        for (subscription_id, session) in
            conn.polkadot_sessions.all_head_sessions.read().await.iter()
        {
            let data = SubscriptionNotification {
                jsonrpc: Version::V2_0,
                method: consts::chain_newHead.to_string(),
                params: SubscriptionNotificationParams {
                    result: data.clone(),
                    // we maintains the subscription id
                    subscription: subscription_id.clone().into(),
                },
            };

            // two level json
            let msg = serialize_elara_api(session, &data);
            let conn = conn.clone();
            tokio::spawn(async move {
                let res = conn.send_message(Message::Text(msg)).await;
                res.map_err(|err| {
                    warn!(
                        "Error occurred when send ChainHead data to `{}`: {:?}",
                        addr, err
                    );
                });
            });
        }
    });
}
