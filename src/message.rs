use crate::session::ISession;

use core::fmt;
pub use jsonrpc_types::{
    Call, Error, Failure, Id, MethodCall, Output, Params, SubscriptionNotification,
    SubscriptionNotificationParams, Success, Value, Version,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::error;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct RequestMessage {
    pub id: String,
    pub chain: String,
    /// A jsonrpc string about request
    pub request: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct ResponseMessage {
    pub id: Option<String>,
    pub chain: Option<String>,
    // an elara error
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorMessage>,
    /// A jsonrpc string about result
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
}

impl ResponseMessage {
    pub fn result_response(id: Option<String>, chain: Option<String>, result: String) -> Self {
        Self {
            id,
            chain,
            error: None,
            result: Some(result),
        }
    }

    pub fn error_response(id: Option<String>, chain: Option<String>, err: ErrorMessage) -> Self {
        Self {
            id,
            chain,
            error: Some(err),
            result: None,
        }
    }
}

/// Elara-kv Error Object.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ErrorMessage {
    /// A Number that indicates the error type that occurred.
    /// This MUST be an integer.
    pub code: ErrorCode,
    /// A String providing a short description of the error.
    /// The message SHOULD be limited to a concise single sentence.
    pub message: String,
}

impl fmt::Display for ErrorMessage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {}", self.code.description(), self.message)
    }
}

impl error::Error for ErrorMessage {}

impl ErrorMessage {
    /// Wraps given `ErrorCode`.
    pub fn new(code: ErrorCode) -> Self {
        Self {
            message: code.description(),
            code,
        }
    }
    /// Creates a new `ParseError` error.
    pub fn parse_error() -> Self {
        Self::new(ErrorCode::ParseError)
    }
    /// Creates a new `ChainNotFound` error.
    pub fn chain_not_found() -> Self {
        Self::new(ErrorCode::ChainNotFound)
    }
}

/// Elara-kv Error Code.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ErrorCode {
    /// Invalid JSON was received by the server.
    /// An error occurred on the server while parsing the JSON text.
    ParseError,
    /// The chain does not exist / is not available.
    ChainNotFound,
    /// Reserved for implementation-defined server-errors.
    ServerError(i64),
}

impl From<i64> for ErrorCode {
    fn from(code: i64) -> Self {
        match code {
            -1 => ErrorCode::ParseError,
            -2 => ErrorCode::ChainNotFound,
            code => ErrorCode::ServerError(code),
        }
    }
}

impl Serialize for ErrorCode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i64(self.code())
    }
}

impl<'de> Deserialize<'de> for ErrorCode {
    fn deserialize<D>(deserializer: D) -> Result<ErrorCode, D::Error>
    where
        D: Deserializer<'de>,
    {
        let code: i64 = Deserialize::deserialize(deserializer)?;
        Ok(ErrorCode::from(code))
    }
}

impl ErrorCode {
    /// Returns integer code value.
    pub fn code(&self) -> i64 {
        match self {
            ErrorCode::ParseError => -1,
            ErrorCode::ChainNotFound => -2,
            ErrorCode::ServerError(code) => *code,
        }
    }

    /// Returns human-readable description.
    pub fn description(&self) -> String {
        let desc = match self {
            ErrorCode::ParseError => "Parse error",
            ErrorCode::ChainNotFound => "Chain not found",
            ErrorCode::ServerError(_) => "Server error",
        };
        desc.to_string()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SubscribedMessage {
    pub id: String,
    pub chain: String,
    /// A jsonrpc string about subscription
    pub data: String,
}

/// Successful response from chain node with subscription id
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubscribedSuccess {
    /// Protocol version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jsonrpc: Option<Version>,
    /// result is subscription id
    pub result: Id,
    /// Correlation id
    pub id: Id,
}

// Note: Altered from jsonrpc_pubsub::SubscriptionId

/// Unique subscription id.
///
/// NOTE Assigning same id to different requests will cause the previous request to be unsubscribed.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(untagged)]
pub enum SubscriptionId {
    /// A numerical ID, represented by a `u64`.
    Number(u64),
    /// A non-numerical ID, for example a hash.
    String(String),
}

impl SubscriptionId {
    /// Parses `core::Value` into unique subscription id.
    pub fn parse_value(val: &Value) -> Option<SubscriptionId> {
        match *val {
            Value::String(ref val) => Some(SubscriptionId::String(val.clone())),
            Value::Number(ref val) => val.as_u64().map(SubscriptionId::Number),
            _ => None,
        }
    }
}

impl From<String> for SubscriptionId {
    fn from(other: String) -> Self {
        SubscriptionId::String(other)
    }
}

impl From<Id> for SubscriptionId {
    fn from(id: Id) -> Self {
        match id {
            Id::Num(val) => SubscriptionId::Number(val),
            Id::Str(val) => SubscriptionId::String(val),
        }
    }
}

impl From<SubscriptionId> for Id {
    fn from(sub: SubscriptionId) -> Self {
        match sub {
            SubscriptionId::Number(val) => Id::Num(val),
            SubscriptionId::String(val) => Id::Str(val),
        }
    }
}

impl From<SubscriptionId> for Value {
    fn from(sub: SubscriptionId) -> Self {
        match sub {
            SubscriptionId::Number(val) => Value::Number(val.into()),
            SubscriptionId::String(val) => Value::String(val),
        }
    }
}

macro_rules! impl_from_num {
    ($num:ty) => {
        impl From<$num> for SubscriptionId {
            fn from(other: $num) -> Self {
                SubscriptionId::Number(other.into())
            }
        }
    };
}

impl_from_num!(u8);
impl_from_num!(u16);
impl_from_num!(u32);
impl_from_num!(u64);

pub fn serialize_success_response<T, S>(session: &S, result: &T) -> String
where
    T: Serialize,
    S: ISession,
{
    let result = serde_json::to_string(&result).expect("serialize a substrate jsonrpc");
    let msg = ResponseMessage {
        id: Some(session.client_id()),
        chain: Some(session.chain_name()),
        error: None,
        result: Some(result),
    };
    serde_json::to_string(&msg).expect("serialize a elara api")
}

pub fn serialize_subscribed_message<T, S>(session: &S, data: &T) -> String
where
    T: Serialize,
    S: ISession,
{
    let data = serde_json::to_string(&data).expect("serialize a substrate jsonrpc");
    let msg = SubscribedMessage {
        id: session.client_id(),
        chain: session.chain_name(),
        data,
    };
    serde_json::to_string(&msg).expect("serialize a elara api")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Result;

    #[test]
    fn test_request_message() -> Result<()> {
        // Some JSON input data as a &str. Maybe this comes from the user.
        let request_data = r#"
{
    "id": "b6c6d0aa16b0f5eb65e6fd87c6ffbba2",
    "chain": "polkadot",
    "request": "{\n\"id\": 141,\n\"jsonrpc\": \"2.0\",\n\"method\": \"state_subscribeStorage\",\n\"params\": [ [\"0x2aeddc77fe58c98d50bd37f1b90840f9cd7f37317cd20b61e9bd46fab87047149c21b6ab44c00eb3127a30e486492921e58f2564b36ab1ca21ff630672f0e76920edd601f8f2b89a\"]]}"
}
"#;

        let v: RequestMessage = serde_json::from_str(request_data)?;
        let _v: MethodCall = serde_json::from_str(&v.request)?;

        Ok(())
    }

    #[test]
    fn test_response_message() -> Result<()> {
        let data = r#"
{
    "id": "b6c6d0aa16b0f5eb65e6fd87c6ffbba2",
    "chain": "polkadot",
    "result": "{\n\"jsonrpc\": \"2.0\",\n\"result\": \"0x91b171bb158e2d3848fa23a9f1c25182fb8e20313b2c1eb49219da7a70ce90c3\",\"id\": 1}"
}
"#;

        let v: ResponseMessage = serde_json::from_str(data)?;
        let _v: Success = serde_json::from_str(&v.result.unwrap())?;

        Ok(())
    }

    #[test]
    fn test_subscribed_message() -> Result<()> {
        let msg = r#"
{
    "id": "b6c6d0aa16b0f5eb65e6fd87c6ffbba2",
    "chain": "polkadot",
    "data": "{\"jsonrpc\": \"2.0\",\n\"method\":\"state_storage\", \n\"params\": {\n\"subscription\": \"ffMpMJgyQt3rmHx8\",\n\t\t\"result\": {\n\t\t  \"block\": \"0x04b67ec2b6ff34ebd58ed95fe9aad1068f805d2519ca8a24b986994b6764f410\",\n\t\t  \"changes\": [\n    [\"0x2aeddc77fe58c98d50bd37f1b90840f9cd7f37317cd20b61e9bd46fab870471456c62bce26605ee05c3c4c795e554a782e59ef5043ca9772f32dfb1ad7de832878d662194193955e\",              null ],[\"0x2aeddc77fe58c98d50bd37f1b90840f943a953ac082e08b6527ce262dbd4abf2e7731c5a045ae2174d185feff2d91e9a5c3c4c795e554a782e59ef5043ca9772f32dfb1ad7de832878d662194193955e\", \"0x3a875e45c13575f66eadb2d60608df9068a90e46ed33723098021e8cedd67d3a09f09f90ad20584949\"]]}}}"
}
"#;

        let v: SubscribedMessage = serde_json::from_str(msg)?;
        let _v: SubscriptionNotification = serde_json::from_str(&v.data)?;

        Ok(())
    }
}
