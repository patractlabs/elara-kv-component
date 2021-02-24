use std::{collections::HashMap, fs, path::PathBuf};

use anyhow::{anyhow, Result};
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
        };
        for (node_name, node_config) in config.nodes.into_iter() {
            let node = node_name
                .parse::<Chain>()
                .map_err(|_| anyhow!("unknown node: {}", node_name))?;
            result.nodes.insert(node, node_config);
        }
        Ok(result)
    }
}

#[derive(Clone, Debug)]
pub struct ServiceConfig {
    pub ws: WsConfig,
    pub nodes: HashMap<Chain, NodeConfig>,
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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WsConfig {
    pub addr: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeConfig {
    pub url: String,
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
    };

    let toml = toml::to_string(&config).unwrap();
    println!("{}", toml);
}
