use crate::polkadot::consts;
use crate::polkadot::service::{
    send_chain_all_head, send_chain_finalized_head, send_chain_new_head,
    send_state_runtime_version, send_state_storage,
};
use crate::rpc_client::RpcClient;
use crate::websocket::{WsConnection, WsConnections};
use async_jsonrpc_client::{NotificationStream, RpcClientError};
use futures::{Stream, StreamExt};
use log::*;
use serde::Serialize;
use std::collections::HashMap;
use std::fmt::Debug;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Polkadot related subscription data
pub struct SubscribedStream {
    storage: NotificationStream,
    version: NotificationStream,
    all_head: NotificationStream,
    new_head: NotificationStream,
    finalized_head: NotificationStream,
}

pub async fn start_polkadot_subscribe(
    client: &RpcClient,
) -> Result<SubscribedStream, RpcClientError> {
    let storage = client
        .subscribe(consts::state_subscribeStorage, None)
        .await?;
    let version = client
        .subscribe(consts::state_subscribeRuntimeVersion, None)
        .await?;
    let all_head = client
        .subscribe(consts::chain_subscribeAllHeads, None)
        .await?;
    let new_head = client
        .subscribe(consts::chain_subscribeNewHeads, None)
        .await?;
    let finalized_head = client
        .subscribe(consts::chain_subscribeFinalizedHeads, None)
        .await?;

    Ok(SubscribedStream {
        storage,
        version,
        all_head,
        new_head,
        finalized_head,
    })
}

async fn send_messages_to_conns<T, S>(
    mut stream: S,
    conns: WsConnections,
    // Do send logic for every connection.
    // It should be non-blocking
    sender: fn(addr: SocketAddr, WsConnection, T),
) where
    T: Serialize + Clone + Debug,
    S: Unpin + Stream<Item = T>,
{
    while let Some(data) = stream.next().await {
        // we get a new data then we send it to all conns
        for (addr, conn) in conns.inner().read().await.iter() {
            let addr = *addr;
            let conn = conn.clone();
            // send one data to n subscription for one connection
            sender(addr, conn, data.clone());
        }
    }
}

// async fn send_message_to_conn<T>(
//     addr: SocketAddr,
//     conn: WsConnection,
//     data: T,
//     // Do send logic for one connection.
//     // It should be non-blocking
//     sender: fn(addr: SocketAddr, WsConnection, T),
// ) where
//     T: Serialize + Clone,
// {
//     // send data to conns
//     sender(addr, conn, data);
// }

impl SubscribedStream {
    // start to push subscription data to all connections in background
    pub fn start(self, conns: WsConnections) {
        let Self {
            storage,
            version,
            all_head,
            new_head,
            finalized_head,
        } = self;

        // we spawn task for every one subscription

        tokio::spawn(send_messages_to_conns(
            storage,
            conns.clone(),
            |addr, conn, data| {
                // TODO:
                match serde_json::value::from_value(data.params.result.clone()) {
                    Ok(data) => send_state_storage(addr, conn, data),

                    Err(err) => {
                        warn!("Receive an illegal data: {}: {}", err, &data)
                    }
                };
            },
        ));

        tokio::spawn(send_messages_to_conns(
            version,
            conns.clone(),
            |addr, conn, data| {
                // TODO:
                match serde_json::value::from_value(data.params.result.clone()) {
                    Ok(data) => send_state_runtime_version(addr, conn, data),

                    Err(err) => {
                        warn!("Receive an illegal subscribed data: {}: {}", err, &data)
                    }
                };
            },
        ));

        tokio::spawn(send_messages_to_conns(
            all_head,
            conns.clone(),
            |addr, conn, data| {
                // TODO:
                match serde_json::value::from_value(data.params.result.clone()) {
                    Ok(data) => send_chain_all_head(addr, conn, data),

                    Err(err) => {
                        warn!("Receive an illegal subscribed data: {}: {}", err, &data)
                    }
                };
            },
        ));

        tokio::spawn(send_messages_to_conns(
            new_head,
            conns.clone(),
            |addr, conn, data| {
                // TODO:
                match serde_json::value::from_value(data.params.result.clone()) {
                    Ok(data) => send_chain_new_head(addr, conn, data),

                    Err(err) => {
                        warn!("Receive an illegal subscribed data: {}: {}", err, &data)
                    }
                };
            },
        ));

        tokio::spawn(send_messages_to_conns(
            finalized_head,
            conns.clone(),
            |addr, conn, data| {
                // TODO:
                match serde_json::value::from_value(data.params.result.clone()) {
                    Ok(data) => send_chain_finalized_head(addr, conn, data),

                    Err(err) => {
                        warn!("Receive an illegal subscribed data: {}: {}", err, &data)
                    }
                };
            },
        ));
    }
}
