pub use async_jsonrpc_client::*;
use serde::{Deserialize, Serialize};

use crate::{session::ISession, Chain};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(untagged)]
pub enum ElaraRequest {
    ElaraSubscriptionRequest(ElaraSubscriptionRequest),
    // TODO: pre config for connection
    ElaraConfig(ElaraConfig),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(untagged)]
pub enum ElaraResponse {
    ElaraSuccess(ElaraSuccessResponse),
    ElaraFailure(ElaraFailureResponse),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ElaraSubscriptionResponse {
    pub id: String,
    pub chain: Chain,
    pub data: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ElaraSubscriptionRequest {
    pub id: String,
    pub chain: Chain,
    pub request: String,
}

impl From<ElaraSubscriptionRequest> for ElaraRequest {
    fn from(resp: ElaraSubscriptionRequest) -> Self {
        Self::ElaraSubscriptionRequest(resp)
    }
}

impl From<ElaraConfig> for ElaraRequest {
    fn from(resp: ElaraConfig) -> Self {
        Self::ElaraConfig(resp)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ElaraConfig {
    pub compression: CompressionType,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CompressionType {
    Gzip,
}

impl ElaraResponse {
    pub fn success(id: String, chain: Chain, result: String) -> Self {
        Self::ElaraSuccess(ElaraSuccessResponse { id, chain, result })
    }

    pub fn failure(id: Option<String>, chain: Option<Chain>, error: Error) -> Self {
        Self::ElaraFailure(ElaraFailureResponse { id, chain, error })
    }
}

impl From<ElaraSuccessResponse> for ElaraResponse {
    fn from(resp: ElaraSuccessResponse) -> Self {
        Self::ElaraSuccess(resp)
    }
}

impl From<ElaraFailureResponse> for ElaraResponse {
    fn from(resp: ElaraFailureResponse) -> Self {
        Self::ElaraFailure(resp)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ElaraSuccessResponse {
    pub id: String,
    pub chain: Chain,
    pub result: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ElaraFailureResponse {
    pub id: Option<String>,
    pub chain: Option<Chain>,
    pub error: Error,
}

pub fn serialize_failure_response<S>(session: &S, error: Error) -> String
where
    S: ISession,
{
    let msg = ElaraFailureResponse {
        id: Some(session.client_id()),
        chain: Some(session.chain()),
        error,
    };
    serde_json::to_string(&msg).expect("serialize a elara api")
}

pub fn serialize_success_response<T, S>(session: &S, result: &T) -> String
where
    T: Serialize,
    S: ISession,
{
    let result = serde_json::to_string(&result).expect("serialize a substrate jsonrpc");
    let msg = ElaraSuccessResponse {
        id: session.client_id(),
        chain: session.chain(),
        result,
    };
    serde_json::to_string(&msg).expect("serialize a elara api")
}

pub fn serialize_subscribed_message<T, S>(session: &S, data: &T) -> String
where
    T: Serialize,
    S: ISession,
{
    let data = serde_json::to_string(&data).expect("serialize a substrate jsonrpc");
    let msg = ElaraSubscriptionResponse {
        id: session.client_id(),
        chain: session.chain(),
        data,
    };
    serde_json::to_string(&msg).expect("serialize a elara api")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_elara_request() {
        let elara_request = r#"
{
    "id": "b6c6d0aa16b0f5eb65e6fd87c6ffbba2",
    "chain": "polkadot",
    "request": "{\n\"id\": 141,\n\"jsonrpc\": \"2.0\",\n\"method\": \"state_subscribeStorage\",\n\"params\": [ [\"0x2aeddc77fe58c98d50bd37f1b90840f9cd7f37317cd20b61e9bd46fab87047149c21b6ab44c00eb3127a30e486492921e58f2564b36ab1ca21ff630672f0e76920edd601f8f2b89a\"]]}"
}
"#;
        let request = serde_json::from_str::<ElaraSubscriptionRequest>(elara_request).unwrap();
        let actual_request = MethodCall::new(
            "state_subscribeStorage",
            Some(Params::Array(vec![
                Value::Array(vec!["0x2aeddc77fe58c98d50bd37f1b90840f9cd7f37317cd20b61e9bd46fab87047149c21b6ab44c00eb3127a30e486492921e58f2564b36ab1ca21ff630672f0e76920edd601f8f2b89a".into()])
            ])),
            Id::Num(141)
        );
        assert_eq!(request.chain, Chain::from("polkadot"));
        let req = serde_json::from_str::<MethodCall>(&request.request).unwrap();
        assert_eq!(req, actual_request);
    }

    #[test]
    fn test_elara_response() {
        let data = r#"
{
    "id": "b6c6d0aa16b0f5eb65e6fd87c6ffbba2",
    "chain": "polkadot",
    "result": "{\n\"jsonrpc\": \"2.0\",\n\"result\": \"0x91b171bb158e2d3848fa23a9f1c25182fb8e20313b2c1eb49219da7a70ce90c3\",\"id\": 1}"
}
"#;

        let response = serde_json::from_str::<ElaraSuccessResponse>(data).unwrap();
        let actual_response = Success::new(
            "0x91b171bb158e2d3848fa23a9f1c25182fb8e20313b2c1eb49219da7a70ce90c3".into(),
            Id::Num(1),
        );
        let resp = serde_json::from_str::<Success>(&response.result).unwrap();
        assert_eq!(resp, actual_response);
    }

    #[test]
    fn test_elara_subscription() {
        let data = r#"
{
    "id": "b6c6d0aa16b0f5eb65e6fd87c6ffbba2",
    "chain": "polkadot",
    "data": "{\"jsonrpc\": \"2.0\",\n\"method\":\"state_storage\", \n\"params\": {\n\"subscription\": \"ffMpMJgyQt3rmHx8\",\n\t\t\"result\": {\n\t\t  \"block\": \"0x04b67ec2b6ff34ebd58ed95fe9aad1068f805d2519ca8a24b986994b6764f410\",\n\t\t  \"changes\": [\n    [\"0x2aeddc77fe58c98d50bd37f1b90840f9cd7f37317cd20b61e9bd46fab870471456c62bce26605ee05c3c4c795e554a782e59ef5043ca9772f32dfb1ad7de832878d662194193955e\",              null ],[\"0x2aeddc77fe58c98d50bd37f1b90840f943a953ac082e08b6527ce262dbd4abf2e7731c5a045ae2174d185feff2d91e9a5c3c4c795e554a782e59ef5043ca9772f32dfb1ad7de832878d662194193955e\", \"0x3a875e45c13575f66eadb2d60608df9068a90e46ed33723098021e8cedd67d3a09f09f90ad20584949\"]]}}}"
}
"#;
        let response = serde_json::from_str::<ElaraSubscriptionResponse>(data).unwrap();
        let params_result = serde_json::json!({
            "block": "0x04b67ec2b6ff34ebd58ed95fe9aad1068f805d2519ca8a24b986994b6764f410",
            "changes": [
                 ["0x2aeddc77fe58c98d50bd37f1b90840f9cd7f37317cd20b61e9bd46fab870471456c62bce26605ee05c3c4c795e554a782e59ef5043ca9772f32dfb1ad7de832878d662194193955e", null],
                 ["0x2aeddc77fe58c98d50bd37f1b90840f943a953ac082e08b6527ce262dbd4abf2e7731c5a045ae2174d185feff2d91e9a5c3c4c795e554a782e59ef5043ca9772f32dfb1ad7de832878d662194193955e", "0x3a875e45c13575f66eadb2d60608df9068a90e46ed33723098021e8cedd67d3a09f09f90ad20584949"],
            ]
        });
        let params =
            SubscriptionNotificationParams::new(Id::Str("ffMpMJgyQt3rmHx8".into()), params_result);
        let actual_subscription = SubscriptionNotification::new("state_storage", params);
        let sub = serde_json::from_str::<SubscriptionNotification>(&response.data).unwrap();
        assert_eq!(sub, actual_subscription);
    }

    #[test]
    fn test_state_runtime_version() {
        let data = r#"
{
    "jsonrpc": "2.0",
    "method": "state_runtimeVersion",
    "params": {
        "result": {
            "apis": [
                ["0xdf6acb689907609b", 3],
                ["0x37e397fc7c91f5e4", 1],
                ["0x40fe3ad401f8959a", 4],
                ["0xd2bc9897eed08f15", 2],
                ["0xf78b278be53f454c", 2],
                ["0xaf2c0297a23e6d3d", 1],
                ["0xed99c5acb25eedf5", 2],
                ["0xcbca25e39f142387", 2],
                ["0x687ad44ad37f03c2", 1],
                ["0xab3c0572291feb8b", 1],
                ["0xbc9d89904f5b923f", 1],
                ["0x37c8bb1350a9a2a8", 1]
            ],
            "authoringVersion": 0,
            "implName": "parity-polkadot",
            "implVersion": 0,
            "specName": "polkadot",
            "specVersion": 28,
            "transactionVersion": 6
        },
        "subscription": "IU2BjP8XYCzKNLbE"
    }
}
"#;

        let response = serde_json::from_str::<SubscriptionNotification>(data).unwrap();
    }
}
