pub mod state;

use serde::{Deserialize, Serialize};
use state::*;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum SubscribedResult {
    StateStorageResult(StateStorageResult),
}
