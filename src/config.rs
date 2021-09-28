use std::collections::HashMap;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::Chain;

#[derive(Clone, Debug)]
pub struct ServiceConfig {
    pub ws: WsConfig,
    pub nodes: HashMap<Chain, NodeConfig>,
    pub client: Option<RpcClientConfig>,
}

impl ServiceConfig {
    pub fn validate(&self, chain: &Chain) -> bool {
        self.nodes.contains_key(chain)
    }

    pub fn parse(toml: String) -> Result<Self> {
        let config = toml::from_str::<ServiceInnerConfig>(toml.as_str())?;

        let mut result = Self {
            ws: config.ws,
            nodes: HashMap::default(),
            client: config.client,
        };
        for (node_name, node_config) in config.nodes.into_iter() {
            result.nodes.insert(Chain::from(node_name), node_config);
        }
        Ok(result)
    }
}

// FIXME: toml-rs don't support serialize HashMap<Chain, NodeConfig>.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct ServiceInnerConfig {
    ws: WsConfig,
    nodes: HashMap<String, NodeConfig>,
    pub client: Option<RpcClientConfig>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WsConfig {
    pub addr: String,
    pub heartbeat_interval_sec: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeConfig {
    pub url: String,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct RpcClientConfig {
    pub timeout_ms: Option<u64>,
    pub max_request_cap: Option<usize>,
    pub max_cap_per_subscription: Option<usize>,
}

#[test]
fn test_toml_config() {
    let mut nodes = HashMap::<String, NodeConfig>::new();
    nodes.insert(
        "polkadot".into(),
        NodeConfig {
            url: "wss://rpc.polkadot.io".into(),
        },
    );
    nodes.insert(
        "kusama".into(),
        NodeConfig {
            url: "wss://kusama-rpc.polkadot.io".into(),
        },
    );
    let config = ServiceInnerConfig {
        ws: WsConfig {
            addr: "localhost:9002".into(),
            heartbeat_interval_sec: Some(10),
        },
        nodes,
        client: None,
    };

    let toml = toml::to_string(&config).unwrap();
    println!("{}", toml);
}
