//! Client related session handlers
//! Set sessions according to user's subscription request

use std::collections::HashSet;

use crate::{
    message::{Error, MethodCall, Params, Success, Value},
    session::{ISessions, NoParamSessions, Session, Sessions},
    substrate::session::{
        AllHeadSessions, FinalizedHeadSessions, GrandpaJustificationSessions, NewHeadSessions,
        RuntimeVersionSessions, StorageKeys, StorageSessions,
    },
};

#[allow(non_snake_case)]
pub fn handle_state_unsubscribeStorage(
    sessions: &mut StorageSessions,
    session: Session,
    request: MethodCall,
) -> Result<Success, Error> {
    handle_unsubscribe(sessions, session, request)
}

#[allow(non_snake_case)]
pub fn handle_state_unsubscribeRuntimeVersion(
    sessions: &mut RuntimeVersionSessions,
    session: Session,
    request: MethodCall,
) -> Result<Success, Error> {
    handle_unsubscribe(sessions, session, request)
}

#[allow(non_snake_case)]
pub fn handle_grandpa_unsubscribeJustifications(
    sessions: &mut GrandpaJustificationSessions,
    session: Session,
    request: MethodCall,
) -> Result<Success, Error> {
    handle_unsubscribe(sessions, session, request)
}

#[allow(non_snake_case)]
pub fn handle_chain_unsubscribeAllHeads(
    sessions: &mut AllHeadSessions,
    session: Session,
    request: MethodCall,
) -> Result<Success, Error> {
    handle_unsubscribe(sessions, session, request)
}

#[allow(non_snake_case)]
pub fn handle_chain_unsubscribeNewHeads(
    sessions: &mut NewHeadSessions,
    session: Session,
    request: MethodCall,
) -> Result<Success, Error> {
    handle_unsubscribe(sessions, session, request)
}

#[allow(non_snake_case)]
pub fn handle_chain_unsubscribeFinalizedHeads(
    sessions: &mut FinalizedHeadSessions,
    session: Session,
    request: MethodCall,
) -> Result<Success, Error> {
    handle_unsubscribe(sessions, session, request)
}

#[allow(non_snake_case)]
#[inline]
pub fn handle_unsubscribe<T>(
    sessions: &mut Sessions<T>,
    _session: Session,
    request: MethodCall,
) -> Result<Success, Error> {
    let params = request.params.unwrap_or_default().parse::<(String,)>()?;
    let subscribed = sessions.remove(&params.0.into()).is_some();
    Ok(Success::new(Value::Bool(subscribed), request.id))
}

#[allow(non_snake_case)]
pub fn handle_state_subscribeStorage(
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
    Ok(Success::new(id.into(), request.id))
}

#[allow(non_snake_case)]
pub fn handle_state_subscribeRuntimeVersion(
    sessions: &mut RuntimeVersionSessions,
    session: Session,
    request: MethodCall,
) -> Result<Success, Error> {
    _handle_no_param_method_call(sessions, session, request)
}

#[allow(non_snake_case)]
pub fn handle_grandpa_subscribeJustifications(
    sessions: &mut RuntimeVersionSessions,
    session: Session,
    request: MethodCall,
) -> Result<Success, Error> {
    _handle_no_param_method_call(sessions, session, request)
}

#[allow(non_snake_case)]
pub fn handle_chain_subscribeAllHeads(
    sessions: &mut AllHeadSessions,
    session: Session,
    request: MethodCall,
) -> Result<Success, Error> {
    _handle_no_param_method_call(sessions, session, request)
}

#[allow(non_snake_case)]
pub fn handle_chain_subscribeNewHeads(
    sessions: &mut NewHeadSessions,
    session: Session,
    request: MethodCall,
) -> Result<Success, Error> {
    _handle_no_param_method_call(sessions, session, request)
}

#[allow(non_snake_case)]
pub fn handle_chain_subscribeFinalizedHeads(
    sessions: &mut FinalizedHeadSessions,
    session: Session,
    request: MethodCall,
) -> Result<Success, Error> {
    _handle_no_param_method_call(sessions, session, request)
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
    // subscription id as result
    Ok(Success::new(id.into(), request.id))
}

/// Check for no params, returns Err if any params
fn expect_no_params(params: &Option<Params>) -> Result<(), Error> {
    match params {
        None => Ok(()),
        Some(Params::Array(ref v)) if v.is_empty() => Ok(()),
        Some(p) => Err(Error::invalid_params_with_details(
            "No parameters were expected",
            p,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::{Id, Params, Success};
    use crate::session::Sessions;
    use crate::Chain;

    #[allow(non_snake_case)]
    #[tokio::test]
    async fn test_state_subscribeStorage() {
        let mut sessions = Sessions::default();

        // subscribe
        let session = Session {
            chain: Chain::from("polkadot"),
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
            chain: Chain::from("polkadot"),
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
