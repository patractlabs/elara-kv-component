use std::{collections::HashMap, fs, path::PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use structopt::StructOpt;

use crate::Chain;

#[derive(Clone, Debug, StructOpt)]
#[structopt(author, about)]
pub struct CliOpts {
    /// Sets a custom config file
    #[structopt(short, long, name = "FILE")]
    pub config: PathBuf,
}

impl CliOpts {
    pub fn init() -> Self {
        CliOpts::from_args()
    }

    pub fn parse(&self) -> Result<ServiceConfig> {
        let toml_str = fs::read_to_string(self.config.as_path())?;
        let config = toml::from_str::<ServiceInnerConfig>(toml_str.as_str())?;

        let mut result = ServiceConfig {
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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeConfig {
    pub url: String,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct RpcClientConfig {
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
        },
        nodes,
        client: None,
    };

    let toml = toml::to_string(&config).unwrap();
    println!("{}", toml);
}
