use thiserror::Error;

#[derive(Error, Debug)]
pub enum ServiceError {
    #[error(transparent)]
    JsonrpcError(#[from] jsonrpc_types::Error),

    #[error(transparent)]
    JsonError(#[from] serde_json::Error),

    #[error(transparent)]
    WsClientError(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("the chain `{0}` is not available")]
    ChainNotSupport(String),
}

pub type Result<T> = std::result::Result<T, ServiceError>;
