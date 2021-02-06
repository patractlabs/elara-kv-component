use serde::{Deserialize, Serialize};

/// storage data as Subscribed data in `result`
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(transparent)]
pub struct GrandpaJustification(Vec<u8>);

impl From<Vec<u8>> for GrandpaJustification {
    fn from(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }
}

impl From<GrandpaJustification> for Vec<u8> {
    fn from(g: GrandpaJustification) -> Self {
        g.0
    }
}
