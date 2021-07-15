#![allow(unused)]

use anyhow::Result;
use elara_kv_component::config::ServiceConfig;
use elara_kv_component::start_server;
use futures::StreamExt;
use std::time::Duration;

mod common;
use common::*;

const URL: &str = "ws://localhost:9002";

#[cfg(feature = "test_unsubscribe")]
#[tokio::test]
async fn test_unsubscribe() -> Result<()> {
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
    let num = 100;
    let total_time = Duration::from_secs(20 * 60);

    println!("test will finish after {:?}", total_time);
    tokio::time::sleep(Duration::from_millis(100)).await;
    run_clients(num, Duration::from_secs(60), Duration::from_secs(20 * 60)).await;
    tokio::time::sleep(total_time).await;
    Ok(())
}

async fn run_clients(num: usize, subscribe_time: Duration, timeout: Duration) {
    for i in 0..num {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let handle = tokio::spawn(async move {
            let mut client = WsClient::connect(URL).await;
            let id = subscribe_chain_head(&mut client)
                .await
                .expect("subscribe panic");
            println!(
                "A new connection {} created, subscribe time: {:?}, connection will finished after {:?}",
                i, subscribe_time, timeout
            );

            let mut subscribed = true;

            tokio::pin! {
                let return_time = tokio::time::sleep(timeout);
                let subscribed_time = tokio::time::sleep(subscribe_time);
            }
            loop {
                let mut reader = client.reader.lock().await;
                tokio::select! {
                    resp = reader.next() => {
                        if let Some(resp) = resp {
                            let resp = resp.unwrap();
                             println!(
                                "client_id: {}, receive server data len: {:?}",
                                i,
                                resp.len()
                            );
                        } else {
                              println!(
                                "client_id: {}, receive None",
                                i,
                            );
                        }
                    }

                    _ = & mut subscribed_time => {
                        subscribed = false;
                        break;
                    }
                }
            }
            if !subscribed {
                unsubscribe_chain_head(&mut client, id.to_string());
                println!("client {} unsubscribed", i);
            }
            return_time.await;
        });
    }
}
