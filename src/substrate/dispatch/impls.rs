use std::fmt::Debug;

use futures::{Stream, StreamExt};
use serde::Serialize;

use crate::{
    rpc_client::NotificationStream,
    substrate::{constants, dispatch::SubscriptionDispatcher, service::*},
    websocket::{WsConnection, WsConnections},
    Chain,
};

#[derive(Clone, Debug)]
pub struct StateStorageDispatcher {
    chain: Chain,
}

impl StateStorageDispatcher {
    pub fn new(chain: Chain) -> Self {
        Self { chain }
    }
}

impl SubscriptionDispatcher for StateStorageDispatcher {
    fn method(&self) -> &'static str {
        constants::state_subscribeStorage
    }

    fn dispatch(&mut self, conns: WsConnections, stream: NotificationStream) {
        let chain = self.chain;
        tokio::spawn(send_messages_to_conns(stream, conns, move |conn, data| {
            match serde_json::value::from_value(data.params.result.clone()) {
                Ok(data) => {
                    let sessions = match chain {
                        Chain::Polkadot => conn.sessions.polkadot_sessions.storage_sessions.clone(),
                        Chain::Kusama => conn.sessions.kusama_sessions.storage_sessions.clone(),
                    };

                    send_state_storage(sessions, conn, data);
                }

                Err(err) => {
                    log::warn!("Receive an illegal subscribed data: {}: {}", err, &data)
                }
            };
        }));
    }
}

#[derive(Clone, Debug)]
pub struct StateRuntimeVersionDispatcher {
    chain: Chain,
}

impl StateRuntimeVersionDispatcher {
    pub fn new(chain: Chain) -> Self {
        Self { chain }
    }
}

impl SubscriptionDispatcher for StateRuntimeVersionDispatcher {
    fn method(&self) -> &'static str {
        constants::state_subscribeRuntimeVersion
    }

    fn dispatch(&mut self, conns: WsConnections, stream: NotificationStream) {
        let chain = self.chain;
        tokio::spawn(send_messages_to_conns(stream, conns, move |conn, data| {
            match serde_json::value::from_value(data.params.result.clone()) {
                Ok(data) => {
                    let sessions = match chain {
                        Chain::Polkadot => conn
                            .sessions
                            .polkadot_sessions
                            .runtime_version_sessions
                            .clone(),
                        Chain::Kusama => conn
                            .sessions
                            .kusama_sessions
                            .runtime_version_sessions
                            .clone(),
                    };

                    send_state_runtime_version(sessions, conn, data);
                }

                Err(err) => {
                    log::warn!("Receive an illegal subscribed data: {}: {}", err, &data)
                }
            };
        }));
    }
}

#[derive(Clone, Debug)]
pub struct ChainNewHeadDispatcher {
    chain: Chain,
}

impl ChainNewHeadDispatcher {
    pub fn new(chain: Chain) -> Self {
        Self { chain }
    }
}

impl SubscriptionDispatcher for ChainNewHeadDispatcher {
    fn method(&self) -> &'static str {
        constants::chain_subscribeNewHeads
    }

    fn dispatch(&mut self, conns: WsConnections, stream: NotificationStream) {
        let chain = self.chain;
        tokio::spawn(send_messages_to_conns(stream, conns, move |conn, data| {
            match serde_json::value::from_value(data.params.result.clone()) {
                Ok(data) => {
                    let sessions = match chain {
                        Chain::Polkadot => {
                            conn.sessions.polkadot_sessions.new_head_sessions.clone()
                        }
                        Chain::Kusama => conn.sessions.kusama_sessions.new_head_sessions.clone(),
                    };

                    send_chain_new_head(sessions, conn, data);
                }

                Err(err) => {
                    log::warn!("Receive an illegal subscribed data: {}: {}", err, &data)
                }
            };
        }));
    }
}

#[derive(Clone, Debug)]
pub struct ChainFinalizedHeadDispatcher {
    chain: Chain,
}

impl ChainFinalizedHeadDispatcher {
    pub fn new(chain: Chain) -> Self {
        Self { chain }
    }
}

impl SubscriptionDispatcher for ChainFinalizedHeadDispatcher {
    fn method(&self) -> &'static str {
        constants::chain_subscribeFinalizedHeads
    }

    fn dispatch(&mut self, conns: WsConnections, stream: NotificationStream) {
        let chain = self.chain;
        tokio::spawn(send_messages_to_conns(stream, conns, move |conn, data| {
            match serde_json::value::from_value(data.params.result.clone()) {
                Ok(data) => {
                    let sessions = match chain {
                        Chain::Polkadot => conn
                            .sessions
                            .polkadot_sessions
                            .finalized_head_sessions
                            .clone(),
                        Chain::Kusama => conn
                            .sessions
                            .kusama_sessions
                            .finalized_head_sessions
                            .clone(),
                    };

                    send_chain_finalized_head(sessions, conn, data);
                }

                Err(err) => {
                    log::warn!("Receive an illegal subscribed data: {}: {}", err, &data)
                }
            };
        }));
    }
}

#[derive(Clone, Debug)]
pub struct ChainAllHeadDispatcher {
    chain: Chain,
}

impl ChainAllHeadDispatcher {
    pub fn new(chain: Chain) -> Self {
        Self { chain }
    }
}

impl SubscriptionDispatcher for ChainAllHeadDispatcher {
    fn method(&self) -> &'static str {
        constants::chain_subscribeAllHeads
    }

    fn dispatch(&mut self, conns: WsConnections, stream: NotificationStream) {
        let chain = self.chain;
        tokio::spawn(send_messages_to_conns(stream, conns, move |conn, data| {
            match serde_json::value::from_value(data.params.result.clone()) {
                Ok(data) => {
                    let sessions = match chain {
                        Chain::Polkadot => {
                            conn.sessions.polkadot_sessions.all_head_sessions.clone()
                        }
                        Chain::Kusama => conn.sessions.kusama_sessions.all_head_sessions.clone(),
                    };

                    send_chain_all_head(sessions, conn, data);
                }

                Err(err) => {
                    log::warn!("Receive an illegal subscribed data: {}: {}", err, &data)
                }
            };
        }));
    }
}

#[derive(Clone, Debug)]
pub struct GrandpaJustificationDispatcher {
    chain: Chain,
}

impl GrandpaJustificationDispatcher {
    pub fn new(chain: Chain) -> Self {
        Self { chain }
    }
}

impl SubscriptionDispatcher for GrandpaJustificationDispatcher {
    fn method(&self) -> &'static str {
        constants::grandpa_subscribeJustifications
    }

    fn dispatch(&mut self, conns: WsConnections, stream: NotificationStream) {
        let chain = self.chain;
        tokio::spawn(send_messages_to_conns(stream, conns, move |conn, data| {
            match serde_json::value::from_value(data.params.result.clone()) {
                Ok(data) => {
                    let sessions = match chain {
                        Chain::Polkadot => conn
                            .sessions
                            .polkadot_sessions
                            .grandpa_justifications
                            .clone(),
                        Chain::Kusama => {
                            conn.sessions.kusama_sessions.grandpa_justifications.clone()
                        }
                    };

                    send_grandpa_justifications(sessions, conn, data);
                }

                Err(err) => {
                    log::warn!("Receive an illegal subscribed data: {}: {}", err, &data)
                }
            };
        }));
    }
}

async fn send_messages_to_conns<T, S>(
    mut stream: S,
    conns: WsConnections,
    // Do send logic for every connection.
    // It should be non-blocking
    sender: impl Fn(WsConnection, T),
) where
    T: Serialize + Clone + Debug,
    S: Unpin + Stream<Item = T>,
{
    while let Some(data) = stream.next().await {
        // we get a new data then we send it to all conns
        for (_, conn) in conns.inner().read().await.iter() {
            let conn = conn.clone();
            // send one data to n subscription for one connection
            sender(conn, data.clone());
        }
    }
}