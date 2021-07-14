#![allow(unused)]

use anyhow::anyhow;
use anyhow::Result;
use elara_kv_component::message::{
    ElaraRequest, ElaraResponse, Id, MethodCall, Params, SubscriptionRequest, Success,
    SuccessResponse, Value,
};
use elara_kv_component::substrate::constants::{
    chain_subscribeAllHeads, chain_unsubscribeAllHeads, state_subscribeStorage,
    state_unsubscribeStorage,
};
use futures::{Sink, SinkExt, Stream, StreamExt};
use once_cell::sync::OnceCell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Error;
use tokio_tungstenite::tungstenite::Message;

pub static ID: OnceCell<AtomicU64> = OnceCell::new();
pub const NODE: &str = "polkadot";

pub fn get_id() -> u64 {
    ID.get().unwrap().fetch_add(1, Ordering::SeqCst)
}

pub fn get_jsonrpc_id() -> Id {
    Id::Num(get_id())
}

pub fn init() {
    ID.set(AtomicU64::from(1)).unwrap();
}

pub async fn get_subscription_id(client: &mut WsClient) -> Result<String, anyhow::Error> {
    let mut reader = client.reader.lock().await;
    let resp = reader.next().await;
    if let Some(Ok(Message::Text(resp))) = resp {
        read_subscription_id(resp)
    } else {
        Err(anyhow!("not a subscription response"))
    }
}

pub fn read_subscription_id(resp: String) -> Result<String, anyhow::Error> {
    let success = serde_json::from_str::<SuccessResponse>(&resp)?;
    let success = serde_json::from_str::<Success>(&success.result)?;
    let subscription_id: String = serde_json::from_value(success.result)?;
    Ok(subscription_id)
}

pub async fn subscribe_state_storage(client: &mut WsClient, params: Option<Params>) {
    let call = MethodCall::new(state_subscribeStorage, params, get_jsonrpc_id());
    let request = serde_json::to_string(&call).unwrap();

    let request = SubscriptionRequest {
        id: 1.to_string(),
        chain: NODE.into(),
        request,
    };

    let request = serde_json::to_string(&ElaraRequest::SubscriptionRequest(request)).unwrap();
    client
        .writer
        .lock()
        .await
        .send(Message::Text(request))
        .await
        .unwrap();
}

pub async fn unsubscribe_state_storage(client: &mut WsClient, id: String) {
    unsubscribe(client, id, state_unsubscribeStorage).await;
}

pub async fn unsubscribe(client: &mut WsClient, id: String, method: &str) {
    let call = MethodCall::new(
        method,
        Some(Params::Array(vec![Value::String(id)])),
        get_jsonrpc_id(),
    );
    let request = serde_json::to_string(&call).unwrap();

    let request = SubscriptionRequest {
        id: 1.to_string(),
        chain: NODE.into(),
        request,
    };

    let request = serde_json::to_string(&ElaraRequest::SubscriptionRequest(request)).unwrap();
    client
        .writer
        .lock()
        .await
        .send(Message::Text(request))
        .await
        .unwrap();
}

pub async fn subscribe_chain_head(client: &mut WsClient) -> Result<String, anyhow::Error> {
    let call = MethodCall::new(chain_subscribeAllHeads, None, get_jsonrpc_id());
    let request = serde_json::to_string(&call).expect("serialize call");

    let request = SubscriptionRequest {
        id: 1.to_string(),
        chain: NODE.into(),
        request,
    };

    let request = serde_json::to_string(&ElaraRequest::SubscriptionRequest(request))
        .expect("serialize subscribe request");
    client
        .writer
        .lock()
        .await
        .send(Message::Text(request))
        .await
        .unwrap();

    get_subscription_id(client).await
}

pub async fn unsubscribe_chain_head(client: &mut WsClient, id: String) {
    unsubscribe(client, id, chain_unsubscribeAllHeads).await;
}

#[derive(Clone)]
pub struct WsClient {
    pub writer: WsWriter,
    pub reader: WsReader,
}

pub type WsWriter = Arc<Mutex<dyn Sink<Message, Error = Error> + Unpin + Send>>;
pub type WsReader = Arc<Mutex<dyn Stream<Item = Result<Message, Error>> + Unpin + Send>>;

impl WsClient {
    pub async fn connect<R>(request: R) -> Self
    where
        R: IntoClientRequest + Unpin,
    {
        let (ws_stream, _) = connect_async(request).await.expect("Failed to connect");
        let (writer, reader) = ws_stream.split();

        Self {
            writer: Arc::from(Mutex::new(writer)),
            reader: Arc::from(Mutex::new(reader)),
        }
    }
}
