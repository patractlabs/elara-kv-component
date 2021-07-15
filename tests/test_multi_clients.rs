#![allow(unused)]

use anyhow::Result;
use elara_kv_component::config::ServiceConfig;
use elara_kv_component::message::Params;
use elara_kv_component::start_server;
use futures::StreamExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

mod common;
use common::*;

#[cfg(feature = "test_multi_clients")]
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
            common::subscribe_state_storage(&mut client, Some(Params::Array(vec![]))).await;

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
