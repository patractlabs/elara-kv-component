use crate::rpc_api::state::StateStorageResult;
use serde::{Deserialize, Serialize};

pub type KafkaStoragePayload = Vec<KafkaStoragePayloadItem>;

// TODO: make sure kafka api from archive side

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct KafkaStoragePayloadItem {
    pub id: u64,
    pub block_num: u64,
    pub hash: String,
    pub is_full: bool,
    pub key: String,
    pub storage: Option<String>,
}

impl From<&KafkaStoragePayload> for StateStorageResult {
    fn from(payload: &KafkaStoragePayload) -> Self {
        Self {
            // assume payload at least have one
            block: payload[0].hash.clone(),
            changes: payload
                .iter()
                .map(|item| (item.key.clone(), item.storage.clone()))
                .collect(),
        }
    }
}
