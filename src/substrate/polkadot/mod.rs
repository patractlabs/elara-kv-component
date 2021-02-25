//! Polkadot related code

pub mod rpc_client;

use crate::message::{
    serialize_success_response, ElaraResponse, Error, Failure, MethodCall, Success,
};
use crate::session::{ISessions, Session, ISession, Sessions};
use crate::substrate::client::{
    handle_chain_subscribeAllHeads, handle_chain_subscribeFinalizedHeads,
    handle_chain_subscribeNewHeads, handle_chain_unsubscribeAllHeads,
    handle_chain_unsubscribeFinalizedHeads, handle_chain_unsubscribeNewHeads,
    handle_grandpa_subscribeJustifications, handle_grandpa_unsubscribeJustifications,
    handle_state_subscribeRuntimeVersion, handle_state_subscribeStorage,
    handle_state_unsubscribeRuntimeVersion, handle_state_unsubscribeStorage, MethodReceiver,
    MethodReceivers, MethodSenders,
};
use crate::substrate::constants;
use crate::substrate::session::{SubscriptionSessions, StorageSessions};
use crate::websocket::{MessageHandler, WsConnection};
use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::RwLock;
use tokio_tungstenite::tungstenite::Message;

pub const NODE_NAME: &str = "polkadot";

pub struct RequestHandler {
    senders: MethodSenders,
}

impl RequestHandler {
    /// make this ws connection subscribing a polkadot node jsonrpc subscription
    pub fn new(conn: WsConnection) -> Self {
        let (senders, receivers) = method_channel();
        handle_subscription_response(conn.clone(), conn.sessions.polkadot_sessions, receivers);
        Self { senders }
    }
}

impl MessageHandler for RequestHandler {
    fn handle(&self, session: Session, request: MethodCall) -> Result<(), ElaraResponse> {
        let sender = self
            .senders
            .get(request.method.as_str())
            .ok_or_else(|| Failure::new(Error::method_not_found(), Some(request.id.clone())))
            .map_err(|err| serde_json::to_string(&err).expect("serialize a failure message"))
            .map_err(|res| ElaraResponse::success(session.client_id.clone(), session.chain, res))?;

        let method = request.method.clone();
        let res = sender.send((session, request));
        if res.is_err() {
            log::warn!("sender channel `{}` is closed", method);
        }
        Ok(())
    }
}

type HandlerFn<S> = fn(&mut S, Session, MethodCall) -> Result<Success, Error>;

pub fn method_channel() -> (MethodSenders, MethodReceivers) {
    let mut receivers = HashMap::new();
    let mut senders = HashMap::new();

    let methods = vec![
        constants::grandpa_subscribeJustifications,
        constants::grandpa_unsubscribeJustifications,
        constants::state_subscribeStorage,
        constants::state_unsubscribeStorage,
        constants::state_subscribeRuntimeVersion,
        constants::state_unsubscribeRuntimeVersion,
        constants::chain_subscribeAllHeads,
        constants::chain_unsubscribeAllHeads,
        constants::chain_subscribeNewHeads,
        constants::chain_unsubscribeNewHeads,
        constants::chain_subscribeFinalizedHeads,
        constants::chain_unsubscribeFinalizedHeads,
        // TODO: now don't support these api
        // constants::author_submitAndWatchExtrinsic,
        // constants::author_unwatchExtrinsic,
    ];

    for method in methods {
        let (sender, receiver) = unbounded_channel::<(Session, MethodCall)>();
        senders.insert(method, sender);
        receivers.insert(method, receiver);
    }
    (senders, receivers)
}

// according to different message to handle different subscription
async fn start_handle<SessionItem, S: Debug + ISessions<SessionItem>>(
    sessions: Arc<RwLock<S>>,
    conn: WsConnection,
    mut receiver: MethodReceiver,
    handle: HandlerFn<S>,
) {
    while let Some((session, request)) = receiver.recv().await {
        // We try to lock it instead of keeping it locked.
        // Because the lock time may be longer.
        let mut sessions = sessions.write().await;
        let res = handle(sessions.borrow_mut(), session.clone(), request);

        let res = res
            .map(|success| serialize_success_response(&session, &success))
            .map_err(|err| serialize_success_response(&session, &err));
        let msg = match res {
            Ok(s) => s,
            Err(s) => s,
        };

        let _res = conn.send_message(Message::Text(msg)).await;
    }
}

// This is a special version to handle state storage
async fn start_state_storage_handle(
    sessions: Arc<RwLock<StorageSessions>>,
    conn: WsConnection,
    mut receiver: MethodReceiver,
) {
    while let Some((session, request)) = receiver.recv().await {
        // We try to lock it instead of keeping it locked.
        // Because the lock time may be longer.
        let mut sessions = sessions.write().await;
        let res = handle_state_subscribeStorage(sessions.borrow_mut(), session.clone(), request.clone());

        let res = res
            .map(|success| serialize_success_response(&session, &success))
            .map_err(|err| serialize_success_response(&session, &err));
        match res {
            Ok(s) => {
                let _res = conn.send_message(Message::Text(s)).await;

                let client = conn.clients.get(&session.chain_name()).expect("wont' panic");
                let client = client.lock().await;
                let params: Vec<Vec<String>> = request.params.unwrap_or_default().parse().expect("won't panic");

                let keys =  match params {
                    arr if arr.len() > 1 => { vec![]}
                    arr if arr.is_empty() || arr[0].is_empty() => { vec![]},
                    arrs => {
                        arrs[0].clone()
                    }
                };

                for key in keys {
                    client.state_get_storage(key).await;
                }

            },
            Err(s) => {
                let _res = conn.send_message(Message::Text(s)).await;
            },
        };

    }
}

/// Start to spawn handler task about subscription jsonrpc in background to response for every subscription.
/// It maintains the sessions for this connection.
fn handle_subscription_response(
    conn: WsConnection,
    sessions: SubscriptionSessions,
    receivers: MethodReceivers,
) {
    for (method, receiver) in receivers.into_iter() {
        match method {
            constants::state_subscribeStorage => {
                tokio::spawn(start_handle(
                    sessions.storage_sessions.clone(),
                    conn.clone(),
                    receiver,
                    handle_state_subscribeStorage,
                ));
            }
            constants::state_unsubscribeStorage => {
                tokio::spawn(start_handle(
                    sessions.storage_sessions.clone(),
                    conn.clone(),
                    receiver,
                    handle_state_unsubscribeStorage,
                ));
            }

            constants::state_subscribeRuntimeVersion => {
                tokio::spawn(start_handle(
                    sessions.runtime_version_sessions.clone(),
                    conn.clone(),
                    receiver,
                    handle_state_subscribeRuntimeVersion,
                ));
            }
            constants::state_unsubscribeRuntimeVersion => {
                tokio::spawn(start_handle(
                    sessions.runtime_version_sessions.clone(),
                    conn.clone(),
                    receiver,
                    handle_state_unsubscribeRuntimeVersion,
                ));
            }

            constants::grandpa_subscribeJustifications => {
                tokio::spawn(start_handle(
                    sessions.grandpa_justifications.clone(),
                    conn.clone(),
                    receiver,
                    handle_grandpa_subscribeJustifications,
                ));
            }
            constants::grandpa_unsubscribeJustifications => {
                tokio::spawn(start_handle(
                    sessions.grandpa_justifications.clone(),
                    conn.clone(),
                    receiver,
                    handle_grandpa_unsubscribeJustifications,
                ));
            }

            constants::chain_subscribeNewHeads => {
                tokio::spawn(start_handle(
                    sessions.new_head_sessions.clone(),
                    conn.clone(),
                    receiver,
                    handle_chain_subscribeNewHeads,
                ));
            }
            constants::chain_unsubscribeNewHeads => {
                tokio::spawn(start_handle(
                    sessions.new_head_sessions.clone(),
                    conn.clone(),
                    receiver,
                    handle_chain_unsubscribeNewHeads,
                ));
            }

            constants::chain_subscribeAllHeads => {
                tokio::spawn(start_handle(
                    sessions.all_head_sessions.clone(),
                    conn.clone(),
                    receiver,
                    handle_chain_subscribeAllHeads,
                ));
            }
            constants::chain_unsubscribeAllHeads => {
                tokio::spawn(start_handle(
                    sessions.all_head_sessions.clone(),
                    conn.clone(),
                    receiver,
                    handle_chain_unsubscribeAllHeads,
                ));
            }

            constants::chain_subscribeFinalizedHeads => {
                tokio::spawn(start_handle(
                    sessions.finalized_head_sessions.clone(),
                    conn.clone(),
                    receiver,
                    handle_chain_subscribeFinalizedHeads,
                ));
            }
            constants::chain_unsubscribeFinalizedHeads => {
                tokio::spawn(start_handle(
                    sessions.finalized_head_sessions.clone(),
                    conn.clone(),
                    receiver,
                    handle_chain_unsubscribeFinalizedHeads,
                ));
            }

            _ => {}
        };
    }
}
