use std::{
    collections::HashMap,
    fs::File,
    io::{self, Read},
};

use serde::Deserialize;
use toml::ser::Error;

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
                !matches!(k, crate::substrate::polkadot::NODE_NAME)
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
