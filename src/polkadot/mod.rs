use std::net::SocketAddr;

use log::*;
use tokio_tungstenite::tungstenite::Message;

use crate::message::{SubscribedData, SubscribedMessage, SubscribedParams, Version};
use crate::rpc_api::chain::ChainHead;
use crate::rpc_api::state::{RuntimeVersion, StateStorageResult};
use crate::rpc_api::SubscribedResult;
use crate::websocket::WsConnection;
use crate::polkadot::session::StorageKeys;

pub mod session;
pub mod util;

pub fn send_state_storage(
    addr: SocketAddr,
    conn: WsConnection,
    data: StateStorageResult,
) {
    tokio::spawn(_send_state_storage(addr, conn, data));
}

pub fn send_state_runtime_version(
    addr: SocketAddr,
    conn: WsConnection,
    data: RuntimeVersion,
) {
    tokio::spawn(_send_state_runtime_version(addr, conn, data));
}

pub fn send_chain_all_head(addr: SocketAddr, conn: WsConnection, data: ChainHead) {
    tokio::spawn(_send_chain_all_head(addr, conn, data));
}

async fn _send_chain_all_head(addr: SocketAddr, conn: WsConnection, data: ChainHead) {
    for (subscription_id, session ) in conn.polkadot_sessions.all_head_sessions.read().await.iter() {
        // TODO: we need to extract the chain type and data type
            let data = SubscribedData {
                jsonrpc: Version::V2_0,
                method: "chain_allHead".to_string(),
                params: SubscribedParams {
                    result: SubscribedResult::ChainAllHead(data.clone()),
                    // we maintains the subscription id
                    subscription: subscription_id.clone(),
                },
            };

            // two level json
            let data = serde_json::to_string(&data)
                .unwrap_or_else(|_| panic!("serialize a subscribed data: {:?}", &data));
            let msg = SubscribedMessage {
                id: session.client_id.clone(),
                chain: session.chain_name.clone(),
                data,
            };
            let msg = serde_json::to_string(&msg)
                .unwrap_or_else(|_| panic!("serialize a subscribed data: {:?}", &msg));

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
}

async fn _send_state_runtime_version(
    addr: SocketAddr,
    conn: WsConnection,
    data: RuntimeVersion,
) {
    for (subscription_id,session) in conn.polkadot_sessions.runtime_version_sessions.read().await.iter() {
            let data = SubscribedData {
                jsonrpc: Version::V2_0,
                method: "state_runtimeVersion".to_string(),
                params: SubscribedParams {
                    result: SubscribedResult::StateRuntimeVersion(data.clone()),
                    // we maintains the subscription id
                    subscription: subscription_id.clone(),
                },
            };

            // two level json
            let data = serde_json::to_string(&data)
                .unwrap_or_else(|_| panic!("serialize a subscribed data: {:?}", &data));
            let msg = SubscribedMessage {
                id: session.client_id.clone(),
                chain: session.chain_name.clone(),
                data,
            };
            let msg = serde_json::to_string(&msg)
                .unwrap_or_else(|_| panic!("serialize a subscribed data: {:?}", &msg));

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
}

async fn _send_state_storage(
    addr: SocketAddr,
    conn: WsConnection,
    data: StateStorageResult,
) {
    for (subscription_id, (session, storage)) in conn.polkadot_sessions.storage_sessions.read().await.iter()
    {
        let data: SubscribedData = match storage {
            StorageKeys::All => {
                SubscribedData {
                    jsonrpc: Version::V2_0,
                    method: "state_storage".to_string(),
                    params: SubscribedParams {
                        result: SubscribedResult::StateStorageResult(data.clone()),
                        // we maintains the subscription id
                        subscription: subscription_id.clone(),
                    },
                }
            }

            StorageKeys::Some(keys) => {
                let filtered_data: Vec<(String, Option<String>)> = data
                    .changes
                    .clone()
                    .into_iter()
                    .filter(|(key, _)| keys.contains(key))
                    .collect();

                SubscribedData {
                    jsonrpc: Version::V2_0,
                    method: "state_storage".to_string(),
                    params: SubscribedParams {
                        result: SubscribedResult::StateStorageResult(
                            StateStorageResult {
                                block: data.block.clone(),
                                changes: filtered_data,
                            },
                        ),
                        // we maintains the subscription id
                        subscription: subscription_id.clone(),
                    },
                }
            }
        };

        // two level json
        let data = serde_json::to_string(&data).expect("serialize a subscribed data");
        let msg = SubscribedMessage {
            id: session.client_id.clone(),
            chain: session.chain_name.clone(),
            data,
        };
        let msg = serde_json::to_string(&msg).expect("serialize a subscribed data");

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
}
