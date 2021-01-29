#[macro_use]
extern crate lazy_static;

pub mod config;
pub mod error;
// TODO: remove kafka
pub mod kafka;
pub mod message;
pub mod polkadot;
pub mod rpc_client;
pub mod session;
pub mod websocket;

mod rpc_api;
