use thiserror::Error;

// TODO: refine error
#[derive(Error, Debug)]
pub enum ServiceError {
    #[error("the chain `{0}` is not available")]
    ChainNotSupport(String),
}

pub type Result<T> = std::result::Result<T, ServiceError>;
