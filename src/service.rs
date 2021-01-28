use crate::client::WsClient;
use crate::config::Config;
use crate::websocket::WsServer;

pub struct Service {
    cfg: Config,
}
//
// impl Service {
//     // start elara kv service
//     pub fn init(cfg: Config) -> Self {
//         let addr = cfg.ws.addr.as_str();
//         let server = WsServer::bind(addr)
//             .await
//             .expect(&*format!("Cannot listen {}", addr));
//         info!("Started ws server at {}", addr);
//
//         // started to subscribe chain node by ws client
//         for (node, cfg) in cfg.nodes.iter() {
//             let client = WsClient::connect(cfg.addr.clone())
//                 .await
//                 .expect(&format!("Cannot connect to {:?}", node));
//         }
//
//         Self { cfg }
//     }
// }
