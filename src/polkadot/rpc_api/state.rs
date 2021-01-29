use crate::polkadot::rpc_api::*;
use serde::{Deserialize, Serialize};

/// storage data as Subscribed data in `result`
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct StateStorage {
    pub block: String,
    // the first elem of tuple is key, the second is storage
    pub changes: Vec<(String, Option<String>)>,
}

impl From<StateStorage> for SubscribedResult {
    fn from(res: StateStorage) -> Self {
        Self::StateStorageResult(res)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeVersion {
    pub spec_name: String,
    pub impl_name: String,
    pub authoring_version: u32,
    pub spec_version: u32,
    pub impl_version: u32,
    pub apis: Vec<(String, u32)>,
}
