use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(transparent)]
pub struct GrandpaJustification(String);

impl From<String> for GrandpaJustification {
    fn from(bytes: String) -> Self {
        Self(bytes)
    }
}

impl From<GrandpaJustification> for String {
    fn from(g: GrandpaJustification) -> Self {
        g.0
    }
}
