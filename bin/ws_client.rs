// use elara_kv_component::client::WsClient;
// use futures::{SinkExt, StreamExt};
// use log::*;
// use tokio::time::Duration;
// use tokio_tungstenite::tungstenite::Error;
// use tokio_tungstenite::tungstenite::Result;
//

#[tokio::main]
async fn main() {
    env_logger::init();
}

// #[tokio::main]
// async fn main() {
//     env_logger::init();
//
//     loop {
//         match handle_client().await {
//             Err(err) => {
//                 warn!("Cannot connect to server");
//                 warn!("Try to connect to server later");
//                 tokio::time::sleep(Duration::from_secs(5)).await;
//             }
//             _ => unreachable!(),
//         }
//     }
// }
//
// async fn handle_client() -> Result<()> {
//     let client = WsClient::connect("ws://localhost:8080".to_string()).await?;
//
//     let mut receiver = client.subscribe();
//     loop {
//         let msg = receiver.recv().await;
//
//         match msg {
//             // server closed?
//             None => return Err(Error::AlreadyClosed),
//
//             Some(Err(Error::Io(err))) => {
//                 return Err(Error::Io(err));
//             }
//
//             Some(Err(err)) => {
//                 warn!("error occurred: {}", err);
//             }
//
//             Some(Ok(msg)) => {
//                 info!("receive a msg: {}", msg);
//             }
//         }
//     }
// }
