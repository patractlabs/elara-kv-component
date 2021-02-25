//! Polkadot related code

pub mod rpc_client;

use crate::message::{
    serialize_failure_response, serialize_subscribed_message, serialize_success_response,
    ElaraResponse, Error, Failure, Id, MethodCall, Params, PubsubTransport,
    SubscriptionNotificationParams, Success,
};
use crate::session::{ISession, ISessions, Session, Sessions};
use crate::substrate::client::{
    handle_chain_subscribeAllHeads, handle_chain_subscribeFinalizedHeads,
    handle_chain_subscribeNewHeads, handle_chain_unsubscribeAllHeads,
    handle_chain_unsubscribeFinalizedHeads, handle_chain_unsubscribeNewHeads,
    handle_grandpa_subscribeJustifications, handle_grandpa_unsubscribeJustifications,
    handle_state_subscribeRuntimeVersion, handle_state_subscribeStorage,
    handle_state_unsubscribeRuntimeVersion, handle_state_unsubscribeStorage,
};
use crate::substrate::rpc::state::StateStorage;
use crate::substrate::session::{StorageSessions, SubscriptionSessions};
use crate::substrate::{constants, MethodReceiver, MethodReceivers, MethodSenders};
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
            .map_err(|err| serialize_failure_response(&session, err));
        let msg = match res {
            Ok(s) => s,
            Err(s) => s,
        };

        let _res = conn.send_message(Message::Text(msg)).await;
    }
}

// This is a special version for handling state storage.
// Because we need the initial changes from chain.
async fn start_state_storage_handle(
    sessions: Arc<RwLock<StorageSessions>>,
    conn: WsConnection,
    mut receiver: MethodReceiver,
) {
    while let Some((session, request)) = receiver.recv().await {
        // We try to lock it instead of keeping it locked.
        // Because the lock time may be longer.
        let mut sessions = sessions.write().await;
        let res =
            handle_state_subscribeStorage(sessions.borrow_mut(), session.clone(), request.clone());
        match res {
            Ok(success) => {
                let msg = serialize_success_response(&session, &success);
                let _res = conn.send_message(Message::Text(msg)).await;

                // we need get the latest storage for these keys
                let clients = conn.clients.clone();
                let client = clients.get(&session.chain()).expect("never panic");
                let stream = {
                    let client = client.read().await;
                    log::info!("start_subscribe {:?}", request.params);
                    client
                        .subscribe(constants::state_subscribeStorage, request.params)
                        .await
                };

                match stream {
                    Err(err) => {
                        log::warn!(
                            "Error occurred when get latest storage for `{}`: {}",
                            session.chain(),
                            err
                        )
                    }

                    Ok(mut stream) => {
                        // we only get the first item for initial changes
                        let latest = stream.next().await.expect("never panic");
                        log::info!("{:?}", latest);
                        let result: Result<StateStorage, _> =
                            serde_json::value::from_value(latest.params.result.clone());

                        // when get the first item, we unsubscribe this.
                        // TODO: need to handle error ?
                        let client = client.clone();
                        tokio::spawn(async move {
                            let client = client.read().await;
                            let res = client
                                .unsubscribe(constants::state_unsubscribeStorage, stream.id.clone())
                                .await;

                            match res {
                                Ok(cancel) => {
                                    log::warn!(
                                        "cannot cancel subscription `{}`, id: {}",
                                        constants::state_unsubscribeStorage,
                                        stream.id
                                    );
                                }
                                Err(err) => {
                                    log::warn!(
                                        "Error occurred when send `{}`, id: {}, error: {}",
                                        constants::state_unsubscribeStorage,
                                        stream.id,
                                        err
                                    )
                                }
                            };
                        });

                        match result {
                            Ok(result) => {
                                let subscription_id: Id =
                                    serde_json::from_value(success.result).expect("never panic");

                                let result = serde_json::to_value(result).expect("never panic");
                                let best_changes = crate::message::SubscriptionNotification::new(
                                    constants::state_storage,
                                    SubscriptionNotificationParams::new(subscription_id, result),
                                );
                                let msg = serialize_subscribed_message(&session, &best_changes);
                                let _res = conn.send_message(Message::Text(msg)).await;
                            }

                            Err(err) => {
                                log::warn!(
                                    "Receive an illegal subscribed data: {}: {}",
                                    err,
                                    &latest
                                )
                            }
                        };
                    }
                }
            }
            Err(err) => {
                let msg = serialize_failure_response(&session, err);
                let _res = conn.send_message(Message::Text(msg)).await;
            }
        };
    }
}

/// Start to spawn handler task about subscription jsonrpc in background to response for every subscription.
/// It maintains the sessions for this connection.
pub fn handle_subscription_response(
    conn: WsConnection,
    sessions: SubscriptionSessions,
    receivers: MethodReceivers,
) {
    for (method, receiver) in receivers.into_iter() {
        match method {
            constants::state_subscribeStorage => {
                tokio::spawn(start_state_storage_handle(
                    sessions.storage_sessions.clone(),
                    conn.clone(),
                    receiver,
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
