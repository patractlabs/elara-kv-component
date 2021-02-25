use std::fmt::Debug;

use async_jsonrpc_client::WsClientError;
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
    Chain,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

pub trait SubscriptionDispatcher {
    fn method(&self) -> &'static str;

    // TODO: refine this
    fn dispatch(&mut self, conns: WsConnections, stream: NotificationStream);
}

#[derive(Clone)]
pub struct SubstrateClient {
    client: Arc<RwLock<RpcClient>>,
    // chain: Chain,
    dispatchers: Arc<Mutex<HashMap<&'static str, Box<dyn SubscriptionDispatcher>>>>,
}

impl SubstrateClient {
    pub fn new(client: Arc<RwLock<RpcClient>>) -> Self {
        Self {
            client,
            // chain,
            dispatchers: Default::default(),
        }
    }

    // pub fn chain(&self) -> Chain {
    //     self.chain
    // }

    pub async fn register_handler(&mut self, handle: Box<dyn SubscriptionDispatcher>) {
        self.dispatchers
            .lock()
            .await
            .insert(handle.method(), handle);
    }

    pub async fn start_handle(&mut self, conns: WsConnections) -> Result<(), WsClientError> {
        for (&method, handle) in self.dispatchers.lock().await.iter_mut() {
            let stream = self.client.read().await.subscribe(method, None).await?;
            let _res = handle.dispatch(conns.clone(), stream);
        }

        Ok(())
    }
}
