use std::{borrow::BorrowMut, collections::HashMap, fmt::Debug, sync::Arc};

use async_jsonrpc_client::Output;
use tokio::sync::{mpsc, RwLock};

use crate::message::{Value};
use crate::substrate::dispatch::DispatcherType;
use crate::substrate::rpc::state::StateStorage;
use crate::substrate::session::StorageKeys;
use crate::substrate::Method;
use crate::{
    message::{
        serialize_failure_response, serialize_subscribed_message, serialize_success_response,
        ElaraResponse, Error, Failure, Id, MethodCall, SubscriptionNotification,
        SubscriptionNotificationParams, Success,
    },
    session::{ISession, ISessions, Session},
    substrate::{
        constants,
        handles::*,
        rpc::state::RuntimeVersion,
        session::{RuntimeVersionSessions, StorageSessions, SubscriptionSessions},
        MethodReceiver, MethodReceivers, MethodSenders,
    },
    websocket::{MessageHandler, WsConnection},
    Chain,
};
use std::collections::HashSet;

/// It handles requests that will cause state(sessions) changes.
pub struct RequestHandler {
    senders: Option<MethodSenders>,
    receivers: Option<MethodReceivers>,
    chain: Chain,
}

impl RequestHandler {
    /// make this ws connection subscribing a polkadot node jsonrpc subscription
    pub fn new(chain: &Chain) -> Self {
        let (senders, receivers) = method_channel();
        Self {
            senders: Some(senders),
            receivers: Some(receivers),
            chain: chain.clone(),
        }
    }
}

impl MessageHandler for RequestHandler {
    fn chain(&self) -> Chain {
        self.chain.clone()
    }

    fn handle_response(&mut self, conn: WsConnection, sessions: SubscriptionSessions) {
        let receivers = self.receivers.take().expect("Only can be called once");
        handle_subscription_response(conn, sessions, receivers)
    }

    fn handle(&self, session: Session, request: MethodCall) -> Result<(), ElaraResponse> {
        if self.senders.is_none() {
            return Ok(());
        }
        let senders = self.senders.as_ref().unwrap();
        let method = Method::from(&request.method);
        let sender = senders
            .get(&method)
            .ok_or_else(|| Failure::new(Error::method_not_found(), Some(request.id.clone())))
            .map_err(|err| serde_json::to_string(&err).expect("serialize a failure message"))
            .map_err(|res| {
                ElaraResponse::success(session.client_id.clone(), session.chain.clone(), res)
            })?;

        let method = request.method.clone();
        let res = sender.send((session, request));
        if res.is_err() {
            log::warn!("sender channel `{}` is closed", method);
        }
        Ok(())
    }

    fn close(&mut self) {
        // we take the senders and drop it. After this, receivers always recv None.
        self.senders.take();
    }
}

type HandlerFn<S> = fn(&mut S, Session, MethodCall) -> Result<Success, Error>;

fn method_channel() -> (MethodSenders, MethodReceivers) {
    let mut receivers = HashMap::new();
    let mut senders = HashMap::new();

    let methods = vec![
        Method::SubscribeStorage,
        Method::UnsubscribeStorage,
        Method::SubscribeRuntimeVersion,
        Method::UnsubscribeRuntimeVersion,
        Method::SubscribeJustifications,
        Method::UnsubscribeJustifications,
        Method::SubscribeAllHeads,
        Method::UnsubscribeAllHeads,
        Method::SubscribeNewHeads,
        Method::UnsubscribeNewHeads,
        Method::SubscribeFinalizedHeads,
        Method::UnsubscribeFinalizedHeads,
    ];

    for method in methods {
        let (sender, receiver) = mpsc::unbounded_channel::<(Session, MethodCall)>();
        senders.insert(method, sender);
        receivers.insert(method, receiver);
    }
    (senders, receivers)
}

// according to different message to handle different subscription
pub async fn start_handle<SessionItem: ISession, S: Debug + ISessions<SessionItem>>(
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
        let _res = conn.send_text(msg).await;
    }
}

/// This is a special version for handling state runtimeVersion.
/// Because we need the initial runtimeVersion from chain.
pub async fn start_state_runtime_version_handle(
    sessions: Arc<RwLock<RuntimeVersionSessions>>,
    conn: WsConnection,
    mut receiver: MethodReceiver,
) {
    while let Some((session, request)) = receiver.recv().await {
        // We try to lock it instead of keeping it locked.
        // Because the lock time may be longer.
        let mut sessions = sessions.write().await;
        let res =
            handle_state_subscribeRuntimeVersion(sessions.borrow_mut(), session.clone(), request);

        match res {
            Err(err) => {
                let msg = serialize_failure_response(&session, err);
                let _res = conn.send_text(msg).await;
            }
            Ok(success) => {
                let msg = serialize_success_response(&session, &success);
                let _res = conn.send_text(msg).await;

                // send the latest runtime version from rpc client
                let subscription_id: Id =
                    serde_json::from_value(success.result).expect("never panic");
                send_latest_runtime_version(conn.clone(), &session, subscription_id).await;
            }
        }
    }
}

async fn send_cached_runtime_version(conn: WsConnection, session: &Session, subscription_id: Id, runtime_version: RuntimeVersion) {
    let data = SubscriptionNotification::new(
        constants::state_runtimeVersion,
        SubscriptionNotificationParams::<RuntimeVersion>::new(
            subscription_id,
            runtime_version,
        ),
    );
    let msg = serialize_subscribed_message(session, &data);
    let _res = conn.send_compression_data(msg).await;
}

async fn send_latest_runtime_version(conn: WsConnection, session: &Session, subscription_id: Id) {
    let client = conn.get_client(&session.chain()).expect("get chain client");
    let client = client.read().await;
    let dispatcher = client
        .dispatcher_ref(constants::state_subscribeRuntimeVersion)
        .expect("get runtime version dispatcher");
    match dispatcher {
        DispatcherType::StateRuntimeVersionDispatcher(s) => {
            let cache = { s.cache.read().await.clone() };
            match cache {
                Some(runtime_version) => {
                    send_cached_runtime_version(conn, session, subscription_id, runtime_version).await;
                }
                None => {
                    // Fetch and cache runtime version from chain node.
                    // Here is the deal with the cold start problem.
                    let res = client.get_runtime_version().await;
                    log::debug!("get the first runtime_version: {:?}", res);
                    match res {
                        Ok(Output::Success(data)) => {
                            let runtime_version: Result<RuntimeVersion, _> = serde_json::from_value(data.result.clone());
                            // validate runtime version format
                            match runtime_version {
                                Err(err) => {
                                    log::warn!("Received a illegal runtime_version: {}", err);
                                    return
                                }
                                Ok(data) => {
                                    let mut cache = s.cache.write().await;
                                    cache.insert(data);
                                }
                            };
                            let data = SubscriptionNotification::new(
                                constants::state_runtimeVersion,
                                SubscriptionNotificationParams::new(subscription_id, data.result),
                            );

                            let msg = serialize_subscribed_message(session, &data);
                            let _res = conn.send_compression_data(msg).await;
                        }

                        Ok(Output::Failure(data)) => {
                            log::warn!("state_getRuntimeVersion failed: {:?}", data);
                        }

                        Err(err) => {
                            log::warn!("send_latest_runtime_version: {}", err);
                        }
                    }
                }
            }
        }
        _ => {
            panic!("get runtime version dispatcher. qed.")
        }
    }
}

/// This is a special version for handling state storage.
/// Because we need return the latest storage immediately.
pub async fn start_state_storage_handle(
    sessions: Arc<RwLock<StorageSessions>>,
    conn: WsConnection,
    mut receiver: MethodReceiver,
) {
    while let Some((session, request)) = receiver.recv().await {
        let mut sessions = sessions.write().await;
        match handle_state_subscribeStorage(sessions.borrow_mut(), session.clone(), request.clone())
        {
            Err(err) => {
                let msg = serialize_failure_response(&session, err);
                let _res = conn.send_text(msg).await;
            }

            Ok((success, keys)) => {
                let msg = serialize_success_response(&session, &success);
                let _res = conn.send_text(msg).await;
                // send the latest storage from cache.
                let subscription_id: Id =
                    serde_json::from_value(success.result).expect("never panic");
                send_latest_storage(conn.clone(), &session, subscription_id, keys).await;
            }
        };
    }
    log::debug!("Connection {} receiver closed", conn.addr());
}

/// Send latest storage cached by client to peer.
/// If it's `All` storage, we send it.
/// If it's `Some` storage, we fetch data from chain node.
async fn send_latest_storage(
    conn: WsConnection,
    session: &Session,
    subscription_id: Id,
    keys: StorageKeys<HashSet<String>>,
) {
    match keys {
        // we get some storage data from chain node
        StorageKeys::Some(keys) => {
            let keys: Vec<Value> = keys.into_iter().map(|key| Value::String(key)).collect();
            // we need get the latest storage from rpc client context
            let client = conn.get_client(&session.chain()).expect("get chain client");
            let client = client.read().await;
            let res = client.query_storage_at(keys).await;

            match res {
                Ok(Output::Success(data)) => {
                    // validate storage format
                    let storage: Result<Vec<StateStorage>, _> =
                        serde_json::from_value(data.result.clone());

                    match storage {
                        Err(err) => {
                            log::warn!("Received a illegal state_storage: {}", err);
                            return;
                        }
                        Ok(storages) if storages.len() != 1 => {
                            log::warn!("Received a illegal state_storage: {:?}", storages);
                            return;
                        }
                        Ok(storages) => {
                            let data = SubscriptionNotification::new(
                                constants::state_storage,
                                SubscriptionNotificationParams::new(
                                    subscription_id,
                                    storages[0].clone(),
                                ),
                            );

                            let msg = serialize_subscribed_message(session, &data);
                            let _res = conn.send_compression_data(msg).await;
                        }
                    }
                }

                Ok(Output::Failure(data)) => {
                    log::warn!("query_storage_at failed: {:?}", data);
                }

                Err(err) => {
                    log::warn!("send_latest_storage: {}", err);
                }
            }
        }

        // we get all storage data from cache
        StorageKeys::All => {
            let client = conn.get_client(&session.chain()).expect("get chain client");
            let client = client.read().await;
            let dispatcher = client
                .dispatcher_ref(constants::state_subscribeStorage)
                .expect("get storage dispatcher");
            match dispatcher {
                DispatcherType::StateStorageDispatcher(s) => {
                    let storage = { s.cache.read().await.clone() };
                    match storage.as_ref() {
                        Some(storage) => {
                            let data = SubscriptionNotification::new(
                                constants::state_storage,
                                SubscriptionNotificationParams::<StateStorage>::new(
                                    subscription_id,
                                    storage.clone(),
                                ),
                            );
                            let msg = serialize_subscribed_message(session, &data);
                            let _res = conn.send_compression_data(msg).await;
                        }
                        None => {
                            // TODO: should we panic?
                        }
                    }
                }
                _ => {
                    panic!("get storage dispatcher. qed.")
                }
            }
        }
    };
}

// TODO: impl a registry for the following pattern
/// Start to spawn handler task about subscription jsonrpc in background to response for every subscription.
/// It maintains the sessions for this connection.
pub fn handle_subscription_response(
    conn: WsConnection,
    sessions: SubscriptionSessions,
    receivers: MethodReceivers,
) {
    for (method, receiver) in receivers {
        match method {
            Method::SubscribeStorage => {
                // TODO
                tokio::spawn(start_state_storage_handle(
                    sessions.storage.clone(),
                    conn.clone(),
                    receiver,
                ));
            }
            Method::UnsubscribeStorage => {
                tokio::spawn(start_handle(
                    sessions.storage.clone(),
                    conn.clone(),
                    receiver,
                    handle_state_unsubscribeStorage,
                ));
            }
            Method::SubscribeRuntimeVersion => {
                tokio::spawn(start_state_runtime_version_handle(
                    sessions.runtime_version.clone(),
                    conn.clone(),
                    receiver,
                ));
            }
            Method::UnsubscribeRuntimeVersion => {
                tokio::spawn(start_handle(
                    sessions.runtime_version.clone(),
                    conn.clone(),
                    receiver,
                    handle_state_unsubscribeRuntimeVersion,
                ));
            }

            Method::SubscribeJustifications => {
                tokio::spawn(start_handle(
                    sessions.grandpa_justifications.clone(),
                    conn.clone(),
                    receiver,
                    handle_grandpa_subscribeJustifications,
                ));
            }
            Method::UnsubscribeJustifications => {
                tokio::spawn(start_handle(
                    sessions.grandpa_justifications.clone(),
                    conn.clone(),
                    receiver,
                    handle_grandpa_unsubscribeJustifications,
                ));
            }

            Method::SubscribeNewHeads => {
                tokio::spawn(start_handle(
                    sessions.new_head.clone(),
                    conn.clone(),
                    receiver,
                    handle_chain_subscribeNewHeads,
                ));
            }
            Method::UnsubscribeNewHeads => {
                tokio::spawn(start_handle(
                    sessions.new_head.clone(),
                    conn.clone(),
                    receiver,
                    handle_chain_unsubscribeNewHeads,
                ));
            }

            Method::SubscribeAllHeads => {
                tokio::spawn(start_handle(
                    sessions.all_head.clone(),
                    conn.clone(),
                    receiver,
                    handle_chain_subscribeAllHeads,
                ));
            }
            Method::UnsubscribeAllHeads => {
                tokio::spawn(start_handle(
                    sessions.all_head.clone(),
                    conn.clone(),
                    receiver,
                    handle_chain_unsubscribeAllHeads,
                ));
            }

            Method::SubscribeFinalizedHeads => {
                tokio::spawn(start_handle(
                    sessions.finalized_head.clone(),
                    conn.clone(),
                    receiver,
                    handle_chain_subscribeFinalizedHeads,
                ));
            }
            Method::UnsubscribeFinalizedHeads => {
                tokio::spawn(start_handle(
                    sessions.finalized_head.clone(),
                    conn.clone(),
                    receiver,
                    handle_chain_unsubscribeFinalizedHeads,
                ));
            }

            _ => {}
        };
    }
}
