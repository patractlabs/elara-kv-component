use crate::message::{SubscribedData, SubscribedMessage, SubscribedParams, Version};
use crate::rpc_api::state::{RuntimeVersion, StateStorageResult};
use crate::rpc_api::SubscribedResult;
use crate::session::{StorageKeys, SubscribedChainDataType};
use crate::websocket::WsConnection;
use std::net::SocketAddr;

use tokio_tungstenite::tungstenite::Message;
use log::*;
use crate::rpc_api::chain::ChainHead;

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

async fn _send_chain_all_head(
    addr: SocketAddr,
    conn: WsConnection,
    data: ChainHead,
) {
    for (subscription_id, (session, ty)) in conn.chain_sessions.read().await.iter() {
        if *ty == SubscribedChainDataType::AllHeads {
            let data = SubscribedData {
                jsonrpc: Some(Version::V2),
                method: "state_runtimeVersion".to_string(),
                params: SubscribedParams {
                    result: SubscribedResult::ChainAllHead(data.clone()),
                    // we maintains the subscription id
                    subscription: subscription_id.clone(),
                },
            };

            // two level json
            let data = serde_json::to_string(&data)
                .expect(&format!("serialize a subscribed data: {:?}", &data));
            let msg = SubscribedMessage {
                id: session.client_id.clone(),
                chain: session.chain_name.clone(),
                data,
            };
            let msg = serde_json::to_string(&msg)
                .expect(&format!("serialize a subscribed data: {:?}", &msg));

            let conn = conn.clone();
            tokio::spawn(async move {
                let res = conn.send_message(Message::Text(msg)).await;
                res.map_err(|err| {
                    warn!(
                        "Error occurred when send RuntimeVersion data to `{}`: {:?}",
                        addr, err
                    );
                });
            });
        }
    }
}

async fn _send_state_runtime_version(
    addr: SocketAddr,
    conn: WsConnection,
    data: RuntimeVersion,
) {
    for (subscription_id, (session, ty)) in conn.chain_sessions.read().await.iter() {
        if *ty == SubscribedChainDataType::RuntimeVersion {
            let data = SubscribedData {
                jsonrpc: Some(Version::V2),
                method: "state_runtimeVersion".to_string(),
                params: SubscribedParams {
                    result: SubscribedResult::StateRuntimeVersion(data.clone()),
                    // we maintains the subscription id
                    subscription: subscription_id.clone(),
                },
            };

            // two level json
            let data = serde_json::to_string(&data)
                .expect(&format!("serialize a subscribed data: {:?}", &data));
            let msg = SubscribedMessage {
                id: session.client_id.clone(),
                chain: session.chain_name.clone(),
                data,
            };
            let msg = serde_json::to_string(&msg)
                .expect(&format!("serialize a subscribed data: {:?}", &msg));

            let conn = conn.clone();
            tokio::spawn(async move {
                let res = conn.send_message(Message::Text(msg)).await;
                res.map_err(|err| {
                    warn!(
                        "Error occurred when send RuntimeVersion data to `{}`: {:?}",
                        addr, err
                    );
                });
            });
        }
    }
}

async fn _send_state_storage(
    addr: SocketAddr,
    conn: WsConnection,
    data: StateStorageResult,
) {
    for (subscription_id, (session, storage)) in conn.storage_sessions.read().await.iter()
    {
        let data: SubscribedData = match storage {
            StorageKeys::All => {
                SubscribedData {
                    jsonrpc: Some(Version::V2),
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
                    jsonrpc: Some(Version::V2),
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
        let data = serde_json::to_string(&data)
            .expect("serialize a subscribed StateStorageResult data");
        let msg = SubscribedMessage {
            id: session.client_id.clone(),
            chain: session.chain_name.clone(),
            data,
        };
        let msg = serde_json::to_string(&msg)
            .expect("serialize a subscribed StateStorageResult data");

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
