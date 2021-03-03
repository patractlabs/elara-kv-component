pub mod cmd;
pub mod message;
pub mod rpc_client;
pub mod session;
pub mod substrate;
pub mod websocket;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, Default)]
#[serde(transparent)]
#[repr(transparent)]
pub struct Chain(String);

impl std::fmt::Display for Chain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<String> for Chain {
    fn from(s: String) -> Self {
        Self(s)
    }
}

// impl std::str::FromStr for Chain {
//     type Err = &'static str;
//
//     fn from_str(s: &str) -> Result<Self, Self::Err> {
//         match s {
//             "polkadot" | "dot" => Ok(Self::Polkadot),
//             "kusama" | "ksm" => Ok(Self::Kusama),
//             _ => Err("unknown node"),
//         }
//     }
// }
