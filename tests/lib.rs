use anyhow::Result;
use elara_kv_component::config::ServiceConfig;
use elara_kv_component::message::{ElaraRequest, Id, MethodCall, Params, SubscriptionRequest};
use elara_kv_component::start_server;
use elara_kv_component::substrate::constants::state_subscribeStorage;
use futures::{Sink, SinkExt, Stream, StreamExt};
use once_cell::sync::OnceCell;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Error;
use tokio_tungstenite::tungstenite::Message;

// const ID: Arc<AtomicU64> = Arc::from(AtomicU64::from(0));

static ID: OnceCell<AtomicU64> = OnceCell::new();

fn get_id() -> u64 {
    ID.get().unwrap().fetch_add(1, Ordering::SeqCst)
}

fn get_jsonrpc_id() -> Id {
    Id::Num(get_id())
}

fn init() {
    ID.set(AtomicU64::from(1)).unwrap();
}

#[cfg(feature = "manual")]
#[tokio::test]
async fn test_multi_clients() -> Result<()> {
    init();

    let toml = r#"
[ws]
addr = "localhost:9002"

[client]
max_request_cap = 256
max_cap_per_subscription = 64

[nodes.polkadot]
url = "wss://rpc.polkadot.io"
    "#
    .to_string();

    let config = ServiceConfig::parse(toml)?;
    tokio::spawn(start_server(config));
    tokio::time::sleep(Duration::from_secs(1)).await;

    let num = 1000;
    println!("{} clients", num);
    run_clients(num, Duration::from_secs(120)).await;
    println!("{} clients created", num);
    tokio::time::sleep(Duration::from_secs(120)).await;
    Ok(())
}

async fn run_clients(num: usize, timeout: Duration) {
    let mut handles = vec![];
    for i in 0..num {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let handle = tokio::spawn(async move {
            let mut client = WsClient::connect("ws://localhost:9002").await;
            println!("A new client, id: {}", i);
            subscribe_state_storage(&mut client, Some(Params::Array(vec![]))).await;

            let exit: Arc<AtomicBool> = Default::default();
            let mut reader = client.reader.lock().await;

            let exit_cline = exit.clone();
            tokio::spawn(async move {
                tokio::time::sleep(timeout).await;
                exit_cline.fetch_or(true, Ordering::SeqCst);
            });

            while let Some(resp) = reader.next().await {
                println!(
                    "client_id: {}, receive server data len: {:?}",
                    i,
                    resp.unwrap().len()
                );
                if exit.load(Ordering::SeqCst) {
                    return;
                }
            }
        });
        handles.push(handle);
    }
}

async fn subscribe_state_storage(client: &mut WsClient, params: Option<Params>) {
    let call = MethodCall::new(state_subscribeStorage, params, get_jsonrpc_id());
    let request = serde_json::to_string(&call).unwrap();

    let request = SubscriptionRequest {
        id: 1.to_string(),
        chain: "polkadot".into(),
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

#[derive(Clone)]
pub struct WsClient {
    pub writer: Arc<Mutex<dyn Sink<Message, Error = Error> + Unpin + Send>>,
    pub reader: Arc<Mutex<dyn Stream<Item = Result<Message, Error>> + Unpin + Send>>,
}

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

// fn create_clients()
