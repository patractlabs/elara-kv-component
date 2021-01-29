//! Client related session handlers
//! Set sessions according to user's subscription request

use super::session::{StorageKeys, StorageSessions};
use crate::message::{Error, MethodCall, Params, Success, Value, Version};
use crate::polkadot::consts;
use crate::polkadot::session::{
    AllHeadSessions, FinalizedHeadSessions, NewHeadSessions, RuntimeVersionSessions,
    WatchExtrinsicSessions,
};
use crate::session::ISessions;
use crate::session::{NoParamSessions, Session, Sessions};
use crate::websocket::WsConnection;

use std::collections::{HashMap, HashSet};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

// Note: we need the session to handle the METHOD call
pub type MethodSender = UnboundedSender<(Session, MethodCall)>;
pub type MethodReceiver = UnboundedReceiver<(Session, MethodCall)>;

pub type MethodSenders = HashMap<&'static str, MethodSender>;
pub type MethodReceivers = HashMap<&'static str, MethodReceiver>;

pub fn polkadot_channel() -> (MethodSenders, MethodReceivers) {
    let mut receivers = HashMap::new();
    let mut senders = HashMap::new();

    // TODO:
    let methods = vec![
        consts::state_subscribeStorage,
        consts::state_subscribeRuntimeVersion,
        consts::chain_subscribeAllHeads,
        consts::chain_subscribeNewHeads,
        consts::chain_subscribeFinalizedHeads,
        consts::author_submitAndWatchExtrinsic,
    ];

    for method in methods {
        let (sender, receiver) = unbounded_channel::<(Session, MethodCall)>();
        senders.insert(method, sender);
        receivers.insert(method, receiver);
    }
    (senders, receivers)
}

// we start to spawn handler task in background to response subscription METHOD
async fn handle_subscribe_response_background(
    conn: WsConnection,
    receivers: MethodReceivers,
) {
    for (method, mut receiver) in receivers.into_iter() {
        let conn = conn.clone();

        let task = match method {
            consts::state_subscribeStorage => {
                async move {
                    while let Some((session, request)) = receiver.recv().await {
                        // TODO:
                    }
                }
            }

            _ => {
                // TODO:
                unimplemented!()
            }
        };
        // TODO:
        tokio::spawn(task);
    }
}

// TODO: refine these as a trait

pub trait ApiHandler<'a> {
    const METHOD: &'static str;
    fn handle(
        &'a mut self,
        session: Session,
        request: MethodCall,
    ) -> Result<Success, Error>;
}

pub struct StorageHandler<'a> {
    pub sessions: &'a mut StorageSessions,
    pub session: Session,
}

// TODO: we should remove the watch when the connection closed
#[allow(non_snake_case)]
pub(crate) fn handle_author_unwatchExtrinsic(
    sessions: &mut WatchExtrinsicSessions,
    session: Session,
    request: MethodCall,
) -> Result<Success, Error> {
    handle_unsubscribe(sessions, session, request)
}

#[allow(non_snake_case)]
pub(crate) fn handle_state_unsubscribeStorage(
    sessions: &mut StorageSessions,
    session: Session,
    request: MethodCall,
) -> Result<Success, Error> {
    handle_unsubscribe(sessions, session, request)
}

#[allow(non_snake_case)]
pub(crate) fn handle_state_unsubscribeRuntimeVersion(
    sessions: &mut RuntimeVersionSessions,
    session: Session,
    request: MethodCall,
) -> Result<Success, Error> {
    handle_unsubscribe(sessions, session, request)
}

#[allow(non_snake_case)]
pub(crate) fn handle_chain_unsubscribeAllHeads(
    sessions: &mut RuntimeVersionSessions,
    session: Session,
    request: MethodCall,
) -> Result<Success, Error> {
    handle_unsubscribe(sessions, session, request)
}

#[allow(non_snake_case)]
pub(crate) fn handle_chain_unsubscribeNewHeads(
    sessions: &mut NewHeadSessions,
    session: Session,
    request: MethodCall,
) -> Result<Success, Error> {
    handle_unsubscribe(sessions, session, request)
}

#[allow(non_snake_case)]
pub(crate) fn handle_chain_unsubscribeFinalizedHeads(
    sessions: &mut FinalizedHeadSessions,
    session: Session,
    request: MethodCall,
) -> Result<Success, Error> {
    handle_unsubscribe(sessions, session, request)
}

#[allow(non_snake_case)]
pub(crate) fn handle_state_subscribeStorage(
    sessions: &mut StorageSessions,
    session: Session,
    request: MethodCall,
) -> Result<Success, Error> {
    let params: Vec<Vec<String>> = request.params.unwrap_or_default().parse()?;
    // TODO: make sure the api semantics
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

            // TODO: try to keep same behavior with substrate
            if len != keys.len() {
                return Err(Error::invalid_params("some keys are invalid"));
            }
            StorageKeys::Some(keys)
        }
    };

    let id = sessions.new_subscription_id();
    sessions.insert(id.clone(), (session, storage_keys));
    Ok(Success {
        jsonrpc: Version::V2_0,
        result: Value::from(id),
        id: request.id,
    })
}

#[allow(non_snake_case)]
pub(crate) fn handle_state_subscribeRuntimeVersion(
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
pub(crate) fn handle_chain_subscribeAllHeads(
    sessions: &mut AllHeadSessions,
    session: Session,
    request: MethodCall,
) -> Result<Success, Error> {
    _handle_chain_subscribeHeads(sessions, session, request)
}

#[allow(non_snake_case)]
pub(crate) fn handle_chain_subscribeNewHeads(
    sessions: &mut NewHeadSessions,
    session: Session,
    request: MethodCall,
) -> Result<Success, Error> {
    _handle_chain_subscribeHeads(sessions, session, request)
}

#[allow(non_snake_case)]
pub(crate) fn handle_chain_subscribeFinalizedHeads(
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
    Ok(Success {
        jsonrpc: Version::V2_0,
        result: Value::Bool(subscribed),
        id: request.id,
    })
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
    Ok(Success {
        jsonrpc: Version::V2_0,
        result: Value::from(id),
        id: request.id,
    })
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

        let success =
            handle_state_subscribeStorage(&mut sessions, session, request).unwrap();

        // unsubscribe
        let session = Session {
            chain_name: "test-net".to_string(),
            client_id: "0x2".to_string(),
        };
        let request = MethodCall {
            jsonrpc: Version::V2_0,
            method: "state_unsubscribeStorage".to_string(),
            params: Some(Params::Array(vec![Value::String(
                success.result.as_str().unwrap().to_string(),
            )])),
            id: Id::Num(2),
        };

        let success = handle_state_unsubscribeStorage(
            &mut sessions,
            session.clone(),
            request.clone(),
        );
        assert_eq!(
            success,
            Ok(Success {
                jsonrpc: Version::V2_0,
                result: Value::Bool(true),
                id: Id::Num(2),
            })
        );

        // unsubscribe again
        let success = handle_state_unsubscribeStorage(&mut sessions, session, request);
        assert_eq!(
            success,
            Ok(Success {
                jsonrpc: Version::V2_0,
                result: Value::Bool(false),
                id: Id::Num(2),
            })
        );
    }
}
