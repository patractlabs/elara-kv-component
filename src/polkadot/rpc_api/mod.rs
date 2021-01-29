pub mod chain;
pub mod state;

use crate::polkadot::rpc_api::chain::ChainHead;
use serde::{Deserialize, Serialize};
use state::*;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum SubscribedResult {
    StateStorageResult(StateStorage),
    StateRuntimeVersion(RuntimeVersion),
    ChainAllHead(ChainHead),
}
