use std::fmt::Debug;

use async_trait::async_trait;

use crate::substrate::rpc::chain::ChainHead;
use crate::substrate::rpc::grandpa::GrandpaJustification;
use crate::substrate::rpc::state::{RuntimeVersion, StateStorage};
use crate::{
    rpc_client::NotificationStream,
    substrate::{constants, dispatch::SubscriptionDispatcher, service::*},
    websocket::WsConnections,
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

#[async_trait]
impl SubscriptionDispatcher for StateStorageDispatcher {
    fn method(&self) -> &'static str {
        constants::state_subscribeStorage
    }

    async fn dispatch(&self, conns: WsConnections, mut stream: NotificationStream) {
        let chain = self.chain.clone();
        let method = self.method();

        tokio::spawn(async move {
            while let Some(data) = stream.next().await {
                // we get a new data then we send it to all conns
                match serde_json::value::from_value::<StateStorage>(data.params.result.clone()) {
                    Ok(data) => {
                        for (_, conn) in conns.inner().read().await.iter() {
                            let sessions = conn
                                .get_sessions(&chain)
                                .await
                                .expect("We check it before subscription");
                            send_state_storage(
                                sessions.storage_sessions.clone(),
                                conn.clone(),
                                data.clone(),
                            );
                        }
                    }

                    Err(err) => {
                        log::warn!("Receive an illegal `{}` data: {}: {}", method, err, &data)
                    }
                };
            }
        });
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

#[async_trait]
impl SubscriptionDispatcher for StateRuntimeVersionDispatcher {
    fn method(&self) -> &'static str {
        constants::state_subscribeRuntimeVersion
    }

    async fn dispatch(&self, conns: WsConnections, mut stream: NotificationStream) {
        let chain = self.chain.clone();
        let method = self.method();

        tokio::spawn(async move {
            while let Some(data) = stream.next().await {
                // we get a new data then we send it to all conns
                match serde_json::value::from_value::<RuntimeVersion>(data.params.result.clone()) {
                    Ok(data) => {
                        for (_, conn) in conns.inner().read().await.iter() {
                            let sessions = conn
                                .get_sessions(&chain)
                                .await
                                .expect("We check it before subscription");
                            send_state_runtime_version(
                                sessions.runtime_version_sessions.clone(),
                                conn.clone(),
                                data.clone(),
                            );
                        }
                    }

                    Err(err) => {
                        log::warn!("Receive an illegal `{}` data: {}: {}", method, err, &data)
                    }
                };
            }
        });
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

#[async_trait]
impl SubscriptionDispatcher for ChainNewHeadDispatcher {
    fn method(&self) -> &'static str {
        constants::chain_subscribeNewHeads
    }

    async fn dispatch(&self, conns: WsConnections, mut stream: NotificationStream) {
        let chain = self.chain.clone();
        let method = self.method();

        tokio::spawn(async move {
            while let Some(data) = stream.next().await {
                // we get a new data then we send it to all conns
                match serde_json::value::from_value::<ChainHead>(data.params.result.clone()) {
                    Ok(data) => {
                        for (_, conn) in conns.inner().read().await.iter() {
                            let sessions = conn
                                .get_sessions(&chain)
                                .await
                                .expect("We check it before subscription");
                            send_chain_new_head(
                                sessions.runtime_version_sessions.clone(),
                                conn.clone(),
                                data.clone(),
                            );
                        }
                    }

                    Err(err) => {
                        log::warn!("Receive an illegal `{}` data: {}: {}", method, err, &data)
                    }
                };
            }
        });
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

#[async_trait]
impl SubscriptionDispatcher for ChainFinalizedHeadDispatcher {
    fn method(&self) -> &'static str {
        constants::chain_subscribeFinalizedHeads
    }

    async fn dispatch(&self, conns: WsConnections, mut stream: NotificationStream) {
        let chain = self.chain.clone();
        let method = self.method();

        tokio::spawn(async move {
            while let Some(data) = stream.next().await {
                // we get a new data then we send it to all conns
                match serde_json::value::from_value::<ChainHead>(data.params.result.clone()) {
                    Ok(data) => {
                        for (_, conn) in conns.inner().read().await.iter() {
                            let sessions = conn
                                .get_sessions(&chain)
                                .await
                                .expect("We check it before subscription");
                            send_chain_finalized_head(
                                sessions.runtime_version_sessions.clone(),
                                conn.clone(),
                                data.clone(),
                            );
                        }
                    }

                    Err(err) => {
                        log::warn!("Receive an illegal `{}` data: {}: {}", method, err, &data)
                    }
                };
            }
        });
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

#[async_trait]
impl SubscriptionDispatcher for ChainAllHeadDispatcher {
    fn method(&self) -> &'static str {
        constants::chain_subscribeAllHeads
    }

    async fn dispatch(&self, conns: WsConnections, mut stream: NotificationStream) {
        let chain = self.chain.clone();
        let method = self.method();

        tokio::spawn(async move {
            while let Some(data) = stream.next().await {
                // we get a new data then we send it to all conns
                match serde_json::value::from_value::<ChainHead>(data.params.result.clone()) {
                    Ok(data) => {
                        for (_, conn) in conns.inner().read().await.iter() {
                            let sessions = conn
                                .get_sessions(&chain)
                                .await
                                .expect("We check it before subscription");
                            send_chain_all_head(
                                sessions.runtime_version_sessions.clone(),
                                conn.clone(),
                                data.clone(),
                            );
                        }
                    }

                    Err(err) => {
                        log::warn!("Receive an illegal `{}` data: {}: {}", method, err, &data)
                    }
                };
            }
        });
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

#[async_trait]
impl SubscriptionDispatcher for GrandpaJustificationDispatcher {
    fn method(&self) -> &'static str {
        constants::grandpa_subscribeJustifications
    }

    async fn dispatch(&self, conns: WsConnections, mut stream: NotificationStream) {
        let chain = self.chain.clone();
        let method = self.method();

        tokio::spawn(async move {
            while let Some(data) = stream.next().await {
                // we get a new data then we send it to all conns
                match serde_json::value::from_value::<GrandpaJustification>(
                    data.params.result.clone(),
                ) {
                    Ok(data) => {
                        for (_, conn) in conns.inner().read().await.iter() {
                            let sessions = conn
                                .get_sessions(&chain)
                                .await
                                .expect("We check it before subscription");
                            send_grandpa_justifications(
                                sessions.runtime_version_sessions.clone(),
                                conn.clone(),
                                data.clone(),
                            );
                        }
                    }

                    Err(err) => {
                        log::warn!("Receive an illegal `{}` data: {}: {}", method, err, &data)
                    }
                };
            }
        });
    }
}
