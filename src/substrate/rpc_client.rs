use std::fmt::Debug;

use async_jsonrpc_client::{WsClientError};
use futures::stream::{Stream, StreamExt};
use serde::Serialize;

use crate::rpc_client::NotificationStream;
use crate::{
    rpc_client::RpcClient,
    substrate::{
        constants,
        service::{
            send_chain_all_head, send_chain_finalized_head, send_chain_new_head,
            send_grandpa_justifications, send_state_runtime_version, send_state_storage,
        },
    },
    websocket::{WsConnection, WsConnections},
};
use std::collections::HashMap;

/// Polkadot related subscription data
pub struct SubscribedStream {
    storage: NotificationStream,
    version: NotificationStream,
    all_head: NotificationStream,
    new_head: NotificationStream,
    finalized_head: NotificationStream,
    grandpa_justifications: NotificationStream,

    // TODO: support register
    inner: HashMap<&'static str, NotificationStream>,
}

pub async fn start_subscribe(client: &RpcClient) -> Result<SubscribedStream, WsClientError> {
    let storage = client
        .subscribe(constants::state_subscribeStorage, None)
        .await?;
    let version = client
        .subscribe(constants::state_subscribeRuntimeVersion, None)
        .await?;
    let grandpa_justifications = client
        .subscribe(constants::grandpa_subscribeJustifications, None)
        .await?;
    let all_head = client
        .subscribe(constants::chain_subscribeAllHeads, None)
        .await?;
    let new_head = client
        .subscribe(constants::chain_subscribeNewHeads, None)
        .await?;
    let finalized_head = client
        .subscribe(constants::chain_subscribeFinalizedHeads, None)
        .await?;

    Ok(SubscribedStream {
        storage,
        version,
        grandpa_justifications,
        all_head,
        new_head,
        finalized_head,
        inner: Default::default(),
    })
}

pub async fn send_messages_to_conns<T, S>(
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

impl SubscribedStream {
    pub fn register_subscription(&mut self, method: &'static str, stream: NotificationStream) -> Option<NotificationStream> {
        self.inner.insert(method, stream)
    }

    // TODO: extract it as trait method
    // start to push subscription data to all connections in background
    pub fn start(self, conns: WsConnections) {
        let Self {
            storage,
            version,
            grandpa_justifications,
            all_head,
            new_head,
            finalized_head,
            inner: _inner
        } = self;


        // we spawn task for every one subscription

        {
            tokio::spawn(send_messages_to_conns(
                storage,
                conns.clone(),
                move |conn, data| {
                    match serde_json::value::from_value(data.params.result.clone()) {
                        Ok(data) => send_state_storage(
                            conn.sessions.polkadot_sessions.storage_sessions.clone(),
                            conn,
                            data,
                        ),

                        Err(err) => {
                            log::warn!("Receive an illegal subscribed data: {}: {}", err, &data)
                        }
                    };
                },
            ));
        }

        {
            tokio::spawn(send_messages_to_conns(
                version,
                conns.clone(),
                move |conn, data| {
                    match serde_json::value::from_value(data.params.result.clone()) {
                        Ok(data) => send_state_runtime_version(
                            conn.sessions
                                .polkadot_sessions
                                .runtime_version_sessions
                                .clone(),
                            conn,
                            data,
                        ),

                        Err(err) => {
                            log::warn!("Receive an illegal subscribed data: {}: {}", err, &data)
                        }
                    };
                },
            ));
        }

        {
            tokio::spawn(send_messages_to_conns(
                grandpa_justifications,
                conns.clone(),
                move |conn, data| {
                    match serde_json::value::from_value(data.params.result.clone()) {
                        Ok(data) => send_grandpa_justifications(
                            conn.sessions
                                .polkadot_sessions
                                .grandpa_justifications
                                .clone(),
                            conn,
                            data,
                        ),

                        Err(err) => {
                            log::warn!("Receive an illegal subscribed data: {}: {}", err, &data)
                        }
                    };
                },
            ));
        }

        {
            tokio::spawn(send_messages_to_conns(
                all_head,
                conns.clone(),
                move |conn, data| {
                    match serde_json::value::from_value(data.params.result.clone()) {
                        Ok(data) => send_chain_all_head(
                            conn.sessions.polkadot_sessions.all_head_sessions.clone(),
                            conn,
                            data,
                        ),

                        Err(err) => {
                            log::warn!("Receive an illegal subscribed data: {}: {}", err, &data)
                        }
                    };
                },
            ));
        }

        {
            tokio::spawn(send_messages_to_conns(
                new_head,
                conns.clone(),
                move |conn, data| {
                    match serde_json::value::from_value(data.params.result.clone()) {
                        Ok(data) => send_chain_new_head(
                            conn.sessions.polkadot_sessions.new_head_sessions.clone(),
                            conn,
                            data,
                        ),

                        Err(err) => {
                            log::warn!("Receive an illegal subscribed data: {}: {}", err, &data)
                        }
                    };
                },
            ));
        }

        {
            tokio::spawn(send_messages_to_conns(
                finalized_head,
                conns,
                move |conn, data| {
                    match serde_json::value::from_value(data.params.result.clone()) {
                        Ok(data) => send_chain_finalized_head(
                            conn.sessions
                                .polkadot_sessions
                                .finalized_head_sessions
                                .clone(),
                            conn,
                            data,
                        ),

                        Err(err) => {
                            log::warn!("Receive an illegal subscribed data: {}: {}", err, &data)
                        }
                    };
                },
            ));
        }
    }
}
