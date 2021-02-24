//! Client related session handlers
//! Set sessions according to user's subscription request

use std::{
    borrow::BorrowMut,
    collections::{HashMap, HashSet},
    fmt::Debug,
    sync::Arc,
};
use tokio::sync::{
    mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    RwLock,
};
use tokio_tungstenite::tungstenite::Message;

use crate::{
    message::{
        serialize_success_response, ElaraResponse, Error, Failure, Id, MethodCall, Params, Success,
        Value,
    },
    session::{ISessions, NoParamSessions, Session, Sessions},
    substrate::{
        constants,
        session::{
            AllHeadSessions, FinalizedHeadSessions, NewHeadSessions, RuntimeVersionSessions,
            StorageKeys, StorageSessions, SubscriptionSessions, WatchExtrinsicSessions,
        },
    },
    websocket::{MessageHandler, WsConnection},
};

// Note: we need the session to handle the method call
pub type MethodSender = UnboundedSender<(Session, MethodCall)>;
pub type MethodReceiver = UnboundedReceiver<(Session, MethodCall)>;

pub type MethodSenders = HashMap<&'static str, MethodSender>;
pub type MethodReceivers = HashMap<&'static str, MethodReceiver>;

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
            .map_err(|res| {
                ElaraResponse::success(session.client_id.clone(), session.chain_name.clone(), res)
            })?;

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

// TODO: now we don't support extrinsic
#[allow(dead_code)]
#[allow(non_snake_case)]
fn handle_author_unwatchExtrinsic(
    sessions: &mut WatchExtrinsicSessions,
    session: Session,
    request: MethodCall,
) -> Result<Success, Error> {
    handle_unsubscribe(sessions, session, request)
}

#[allow(non_snake_case)]
fn handle_state_unsubscribeStorage(
    sessions: &mut StorageSessions,
    session: Session,
    request: MethodCall,
) -> Result<Success, Error> {
    handle_unsubscribe(sessions, session, request)
}

#[allow(non_snake_case)]
fn handle_state_unsubscribeRuntimeVersion(
    sessions: &mut RuntimeVersionSessions,
    session: Session,
    request: MethodCall,
) -> Result<Success, Error> {
    handle_unsubscribe(sessions, session, request)
}

#[allow(non_snake_case)]
fn handle_grandpa_unsubscribeJustifications(
    sessions: &mut RuntimeVersionSessions,
    session: Session,
    request: MethodCall,
) -> Result<Success, Error> {
    handle_unsubscribe(sessions, session, request)
}

#[allow(non_snake_case)]
fn handle_chain_unsubscribeAllHeads(
    sessions: &mut RuntimeVersionSessions,
    session: Session,
    request: MethodCall,
) -> Result<Success, Error> {
    handle_unsubscribe(sessions, session, request)
}

#[allow(non_snake_case)]
fn handle_chain_unsubscribeNewHeads(
    sessions: &mut NewHeadSessions,
    session: Session,
    request: MethodCall,
) -> Result<Success, Error> {
    handle_unsubscribe(sessions, session, request)
}

#[allow(non_snake_case)]
fn handle_chain_unsubscribeFinalizedHeads(
    sessions: &mut FinalizedHeadSessions,
    session: Session,
    request: MethodCall,
) -> Result<Success, Error> {
    handle_unsubscribe(sessions, session, request)
}

#[allow(non_snake_case)]
fn handle_state_subscribeStorage(
    sessions: &mut StorageSessions,
    session: Session,
    request: MethodCall,
) -> Result<Success, Error> {
    let params: Vec<Vec<String>> = request.params.unwrap_or_default().parse()?;
    let storage_keys = match params {
        arr if arr.len() > 1 => {
            return Err(Error::invalid_params("more than one param"));
        }
        arr if arr.is_empty() || arr[0].is_empty() => StorageKeys::All,
        arrs => {
            let arr = &arrs[0];
            let len = arr.len();
            let keys = arr
                .iter()
                .map(|v| v.to_string())
                .collect::<HashSet<String>>();

            if len != keys.len() {
                return Err(Error::invalid_params("some keys are invalid"));
            }
            StorageKeys::Some(keys)
        }
    };

    let id = sessions.new_subscription_id();
    sessions.insert(id.clone(), (session, storage_keys));
    let result = match id {
        Id::Num(id) => Value::Number(id.into()),
        Id::Str(id) => Value::String(id),
    };
    Ok(Success::new(result, request.id))
}

#[allow(non_snake_case)]
fn handle_state_subscribeRuntimeVersion(
    sessions: &mut RuntimeVersionSessions,
    session: Session,
    request: MethodCall,
) -> Result<Success, Error> {
    _handle_no_param_method_call(sessions, session, request)
}

#[allow(non_snake_case)]
fn handle_grandpa_subscribeJustifications(
    sessions: &mut RuntimeVersionSessions,
    session: Session,
    request: MethodCall,
) -> Result<Success, Error> {
    _handle_no_param_method_call(sessions, session, request)
}

/// Check for no params, returns Err if any params
pub fn expect_no_params(params: &Option<Params>) -> Result<(), Error> {
    match params {
        None => Ok(()),
        Some(Params::Array(ref v)) if v.is_empty() => Ok(()),
        Some(p) => Err(Error::invalid_params_with_details(
            "No parameters were expected",
            p,
        )),
    }
}

#[allow(non_snake_case)]
fn handle_chain_subscribeAllHeads(
    sessions: &mut AllHeadSessions,
    session: Session,
    request: MethodCall,
) -> Result<Success, Error> {
    _handle_chain_subscribeHeads(sessions, session, request)
}

#[allow(non_snake_case)]
fn handle_chain_subscribeNewHeads(
    sessions: &mut NewHeadSessions,
    session: Session,
    request: MethodCall,
) -> Result<Success, Error> {
    _handle_chain_subscribeHeads(sessions, session, request)
}

#[allow(non_snake_case)]
fn handle_chain_subscribeFinalizedHeads(
    sessions: &mut FinalizedHeadSessions,
    session: Session,
    request: MethodCall,
) -> Result<Success, Error> {
    _handle_chain_subscribeHeads(sessions, session, request)
}

#[allow(non_snake_case)]
#[inline]
fn handle_unsubscribe<T>(
    sessions: &mut Sessions<T>,
    _session: Session,
    request: MethodCall,
) -> Result<Success, Error> {
    let params = request.params.unwrap_or_default().parse::<(String,)>()?;
    let subscribed = sessions.remove(&params.0.into()).is_some();
    Ok(Success::new(Value::Bool(subscribed), request.id))
}

#[allow(non_snake_case)]
#[inline]
fn _handle_no_param_method_call(
    sessions: &mut NoParamSessions,
    session: Session,
    request: MethodCall,
) -> Result<Success, Error> {
    expect_no_params(&request.params)?;

    let id = sessions.new_subscription_id();
    sessions.insert(id.clone(), session);
    let result = match id {
        Id::Num(id) => Value::Number(id.into()),
        Id::Str(id) => Value::String(id),
    };
    Ok(Success::new(result, request.id))
}

#[allow(non_snake_case)]
#[inline]
fn _handle_chain_subscribeHeads(
    sessions: &mut NoParamSessions,
    session: Session,
    request: MethodCall,
) -> Result<Success, Error> {
    _handle_no_param_method_call(sessions, session, request)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::{Id, Params, Success};
    use crate::session::Sessions;

    #[allow(non_snake_case)]
    #[tokio::test]
    async fn test_state_subscribeStorage() {
        let mut sessions = Sessions::default();

        // subscribe
        let session = Session {
            chain_name: "test-net".to_string(),
            client_id: "0x1".to_string(),
        };

        let request: MethodCall = serde_json::from_str(
            r##"
{
  "jsonrpc": "2.0",
  "method": "state_subscribeStorage",
  "params": [],
  "id": 1
}
        "##,
        )
        .unwrap();

        let success = handle_state_subscribeStorage(&mut sessions, session, request).unwrap();

        // unsubscribe
        let session = Session {
            chain_name: "test-net".to_string(),
            client_id: "0x2".to_string(),
        };
        let request = MethodCall::new(
            "state_unsubscribeStorage",
            Some(Params::Array(vec![Value::String(
                success.result.as_str().unwrap().to_string(),
            )])),
            Id::Num(2),
        );

        let success =
            handle_state_unsubscribeStorage(&mut sessions, session.clone(), request.clone());
        assert_eq!(success, Ok(Success::new(Value::Bool(true), Id::Num(2))));

        // unsubscribe again
        let success = handle_state_unsubscribeStorage(&mut sessions, session, request);
        assert_eq!(success, Ok(Success::new(Value::Bool(false), Id::Num(2))));
    }
}
