use serde::{Deserialize, Serialize};

pub mod chain {
    use super::*;

    #[derive(Clone, Debug, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ChainHead {
        pub digest: DigestLogs,
        pub extrinsics_root: String,
        pub number: String,
        pub parent_hash: String,
        pub state_root: String,
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct DigestLogs {
        logs: Vec<String>,
    }
}

pub mod grandpa {
    use super::*;

    #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
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
}

pub mod state {
    use super::*;

    #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
    pub struct StateStorage {
        pub block: String,
        pub changes: Vec<(String, Option<String>)>,
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct RuntimeVersion {
        pub spec_name: String,
        pub impl_name: String,
        pub authoring_version: u32,
        pub spec_version: u32,
        pub impl_version: u32,
        pub apis: Vec<(String, u32)>,
        pub transaction_version: u32,
    }
}
