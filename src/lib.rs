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

impl From<&str> for Chain {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}
