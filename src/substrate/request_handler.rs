use std::{borrow::BorrowMut, collections::HashMap, fmt::Debug, sync::Arc};

use async_jsonrpc_client::Output;
use tokio::sync::{mpsc, RwLock};

use crate::substrate::dispatch::{DispatcherType};
use crate::substrate::Method;
use crate::{
    message::{
        serialize_failure_response, serialize_subscribed_message, serialize_success_response,
        ElaraResponse, Error, Failure, Id, MethodCall, SubscriptionNotification,
        SubscriptionNotificationParams, Success,
    },
    rpc_client::ArcRpcClient,
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

// This is a special version for handling state runtimeVersion.
// Because we need the initial runtimeVersion from chain.
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
                let client = conn.get_client(&session.chain()).expect("get chain client");
                send_latest_runtime_version(conn.clone(), &session, client, subscription_id).await;
            }
        }
    }
}

async fn send_latest_runtime_version(
    conn: WsConnection,
    session: &Session,
    client: ArcRpcClient,
    subscription_id: Id,
) {
    let client = client.read().await;
    let res = client.get_runtime_version().await;

    match res {
        Ok(Output::Success(data)) => {
            let runtime_version: Result<RuntimeVersion, _> = serde_json::from_value(data.result);
            let runtime_version = match runtime_version {
                Ok(runtime_version) => runtime_version,
                Err(err) => {
                    log::warn!("Received a illegal runtime_version: {}", err);
                    return;
                }
            };
            let result = serde_json::to_value(runtime_version).expect("serialize runtime_version");
            let data = SubscriptionNotification::new(
                constants::state_runtimeVersion,
                SubscriptionNotificationParams::new(subscription_id, result),
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

// This is a special version for handling state storage.
// Because we need return the latest storage immediately.
pub async fn start_state_storage_handle(
    sessions: Arc<RwLock<StorageSessions>>,
    conn: WsConnection,
    mut receiver: MethodReceiver,
) {
    while let Some((session, request)) = receiver.recv().await {
        let mut sessions = sessions.write().await;
        let res =
            handle_state_subscribeStorage(sessions.borrow_mut(), session.clone(), request.clone());
        match res {
            Err(err) => {
                let msg = serialize_failure_response(&session, err);
                let _res = conn.send_text(msg).await;
            }

            Ok(success) => {
                let msg = serialize_success_response(&session, &success);
                let _res = conn.send_text(msg).await;
                // send the latest storage from cache.
                let subscription_id: Id =
                    serde_json::from_value(success.result).expect("never panic");
                send_latest_storage(conn.clone(), &session, subscription_id).await;
            }
        };
    }
    log::debug!("Connection {} receiver closed", conn.addr());
}

async fn send_latest_storage(conn: WsConnection, session: &Session, subscription_id: Id) {
    // we need get the latest storage for these keys.
    let client = conn.get_client(&session.chain()).expect("get chain client");
    let client = client.read().await;
    let ctx = client.ctx.as_ref().expect("get client context");
    let dispatchers = ctx.handler.dispatchers();
    let dispatcher = dispatchers
        .get(constants::state_subscribeStorage)
        .expect("get storage dispatcher");
    match dispatcher.as_ref() {
        DispatcherType::StateStorageDispatcher(s) => {
            let storage = s.cache_cur_storage.read().await;
            match storage.as_ref() {
                Some(storage) => {
                    let result = serde_json::to_value(storage).expect("serialize runtime_version");
                    let data = SubscriptionNotification::new(
                        constants::state_runtimeVersion,
                        SubscriptionNotificationParams::new(subscription_id, result),
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
