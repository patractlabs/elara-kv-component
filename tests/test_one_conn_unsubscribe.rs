#![allow(unused)]

use anyhow::Result;
use elara_kv_component::config::ServiceConfig;
use elara_kv_component::start_server;
use futures::StreamExt;
use std::time::Duration;

mod common;
use common::*;
use elara_kv_component::message::Params;

const URL: &str = "ws://localhost:9002";

#[cfg(feature = "test_one_conn_unsubscribe")]
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
    let num: usize = 100;
    tokio::time::sleep(Duration::from_millis(100)).await;
    do_subscribe(num, Duration::from_secs(20)).await;
    Ok(())
}

async fn do_subscribe(num: usize, subscribe_time: Duration) {
    let mut client = WsClient::connect(URL).await;
    for i in 0..num {
        tokio::time::sleep(Duration::from_millis(10)).await;
        // subscribe_chain_head(&mut client).await;
        subscribe_state_storage(&mut client, Some(Params::Array(vec![]))).await;
        println!(
            "A new subscription {} created, subscribe time: {:?}",
            i, subscribe_time
        );
    }

    let mut subscribed = true;
    let mut id_list: Vec<String> = vec![];
    let mut count = 0;
    loop {
        let mut reader = client.reader.lock().await;
        count += 1;
        println!("loop: {}", count);
        tokio::select! {
            resp = reader.next() => {
                if let Some(resp) = resp {
                    let resp = resp.unwrap();
                     println!(
                        "receive server data len: {:?}",
                        resp.len()
                    );
                    let id = read_subscription_id(resp.to_string());
                    if let Ok(id) = id {
                        println!("subscription id: {}, totoal num: {}", id, id_list.len());
                        id_list.push(id);
                    }
                } else {
                    println!("receive None");
                }
            }

            _ = tokio::time::sleep(subscribe_time) => {
                println!("subscribed_time end");
                subscribed = false;
                break;
            }
        }
    }
    if !subscribed {
        for id in id_list {
            unsubscribe_state_storage(&mut client, id.to_string());
            // unsubscribe_chain_head(&mut client, id.to_string());
            println!("subscription {} unsubscribed", id);
        }
    }
    println!("end subscription");
}
