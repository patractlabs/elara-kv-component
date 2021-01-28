pub mod chain;
pub mod state;

use serde::{Deserialize, Serialize};
use state::*;
use crate::rpc_api::chain::ChainHead;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum SubscribedResult {
    StateStorageResult(StateStorageResult),
    StateRuntimeVersion(RuntimeVersion),
    ChainAllHead(ChainHead),
}
