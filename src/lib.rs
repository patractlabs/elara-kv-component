pub mod cmd;
pub mod message;
pub mod rpc_client;
pub mod session;
pub mod substrate;
pub mod websocket;

use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Chain {
    Polkadot,
    Kusama,
}

impl Default for Chain {
    fn default() -> Self {
        Self::Polkadot
    }
}

impl std::fmt::Display for Chain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Polkadot => f.write_str("polkadot"),
            Self::Kusama => f.write_str("kusama"),
        }
    }
}

impl std::str::FromStr for Chain {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "polkadot" | "dot" => Ok(Self::Polkadot),
            "kusama" | "ksm" => Ok(Self::Kusama),
            _ => Err("unknown node"),
        }
    }
}
