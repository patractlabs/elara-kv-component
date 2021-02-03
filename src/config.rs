use serde::Deserialize;
use toml::ser::Error;

use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::io::prelude::*;

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub ws: WsConfig,
    pub nodes: HashMap<String, NodeConfig>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WsConfig {
    pub addr: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NodeConfig {
    pub addr: String,
}

impl Config {
    pub fn validate(self) -> Result<Self, Error> {
        let invalid_nodes = self
            .nodes
            .iter()
            .filter(|(k, _v)| {
                let k = k.as_str();
                match k {
                    crate::polkadot::NODE_NAME | crate::kusama::NODE_NAME => false,
                    _ => true,
                }
            })
            .collect::<HashMap<_, _>>();

        if !invalid_nodes.is_empty() {
            Err(Error::Custom(format!(
                "Some chain nodes are invalid: {:?}",
                invalid_nodes
            )))
        } else if self.nodes.is_empty() {
            Err(Error::Custom(
                "chain nodes config at least need one node".to_string(),
            ))
        } else {
            Ok(self)
        }
    }
}

pub fn load_config(path: &str) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut res = String::new();
    let _ = file.read_to_string(&mut res)?;
    Ok(res)
}
