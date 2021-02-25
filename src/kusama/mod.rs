use crate::message::*;
use crate::session::Session;
use crate::substrate::MethodSenders;
use crate::websocket::{MessageHandler, WsConnection};

pub const NODE_NAME: &str = "kusama";

pub use crate::polkadot::register_subscriptions;
use crate::polkadot::{handle_subscription_response, method_channel};

pub struct RequestHandler {
    senders: MethodSenders,
}

impl RequestHandler {
    /// make this ws connection subscribing a kusama node jsonrpc subscription
    pub fn new(conn: WsConnection) -> Self {
        let (senders, receivers) = method_channel();
        handle_subscription_response(conn.clone(), conn.sessions.kusama_sessions, receivers);
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
