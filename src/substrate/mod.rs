#![allow(non_upper_case_globals)]

pub mod dispatch;
pub mod handles;
pub mod request_handler;
pub mod rpc;
pub mod service;
pub mod session;

use std::collections::HashMap;

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::{message::MethodCall, session::Session};

// Note: we need the session to handle the method call
pub type MethodSender = UnboundedSender<(Session, MethodCall)>;
pub type MethodReceiver = UnboundedReceiver<(Session, MethodCall)>;

pub type MethodSenders = HashMap<Method, MethodSender>;
pub type MethodReceivers = HashMap<Method, MethodReceiver>;

/// All substrate subscription methods that we supported.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Method {
    NotFound,

    SubscribeStorage,
    UnsubscribeStorage,

    SubscribeRuntimeVersion,
    UnsubscribeRuntimeVersion,

    SubscribeJustifications,
    UnsubscribeJustifications,

    SubscribeAllHeads,
    UnsubscribeAllHeads,

    SubscribeNewHeads,
    UnsubscribeNewHeads,

    SubscribeFinalizedHeads,
    UnsubscribeFinalizedHeads,
}

impl Method {
    pub fn to_option(&self) -> Option<Method> {
        match self {
            Method::NotFound => None,
            other => Some(*other),
        }
    }
}

impl From<&str> for Method {
    fn from(s: &str) -> Self {
        use constants::*;
        use Method::*;
        match s {
            state_subscribeStorage => SubscribeStorage,
            state_unsubscribeStorage => UnsubscribeStorage,

            state_subscribeRuntimeVersion | chain_subscribeRuntimeVersion => {
                SubscribeRuntimeVersion
            }
            state_unsubscribeRuntimeVersion | chain_unsubscribeRuntimeVersion => {
                UnsubscribeRuntimeVersion
            }

            chain_subscribeAllHeads => SubscribeAllHeads,
            chain_unsubscribeAllHeads => UnsubscribeAllHeads,

            chain_subscribeNewHeads | chain_subscribeNewHead | subscribe_newHead => {
                SubscribeNewHeads
            }
            chain_unsubscribeNewHeads | chain_unsubscribeNewHead | unsubscribe_newHead => {
                UnsubscribeNewHeads
            }

            chain_subscribeFinalizedHeads | chain_subscribeFinalisedHeads => {
                SubscribeFinalizedHeads
            }
            chain_unsubscribeFinalizedHeads | chain_unsubscribeFinalisedHeads => {
                UnsubscribeFinalizedHeads
            }
            _ => NotFound,
        }
    }
}

impl From<String> for Method {
    fn from(s: String) -> Self {
        Self::from(&s)
    }
}

impl From<&String> for Method {
    fn from(s: &String) -> Self {
        Self::from(s.as_str())
    }
}

pub mod constants {
    pub const state_getRuntimeVersion: &str = "state_getRuntimeVersion";
    pub const system_health: &str = "system_health";

    pub const state_subscribeStorage: &str = "state_subscribeStorage";
    pub const state_unsubscribeStorage: &str = "state_unsubscribeStorage";
    pub const state_storage: &str = "state_storage";

    pub const state_subscribeRuntimeVersion: &str = "state_subscribeRuntimeVersion";
    pub const state_unsubscribeRuntimeVersion: &str = "state_unsubscribeRuntimeVersion";
    pub const chain_subscribeRuntimeVersion: &str = "chain_subscribeRuntimeVersion";
    pub const chain_unsubscribeRuntimeVersion: &str = "chain_unsubscribeRuntimeVersion";
    pub const state_runtimeVersion: &str = "state_runtimeVersion";

    pub const grandpa_subscribeJustifications: &str = "grandpa_subscribeJustifications";
    pub const grandpa_unsubscribeJustifications: &str = "grandpa_unsubscribeJustifications";
    pub const grandpa_justifications: &str = "grandpa_justifications";

    pub const chain_subscribeAllHeads: &str = "chain_subscribeAllHeads";
    pub const chain_unsubscribeAllHeads: &str = "chain_unsubscribeAllHeads";
    pub const chain_allHead: &str = "chain_allHead";

    pub const chain_subscribeNewHeads: &str = "chain_subscribeNewHeads";
    pub const chain_subscribeNewHead: &str = "chain_subscribeNewHead";
    pub const subscribe_newHead: &str = "subscribe_newHead";
    pub const chain_unsubscribeNewHeads: &str = "chain_unsubscribeNewHeads";
    pub const chain_unsubscribeNewHead: &str = "chain_unsubscribeNewHead";
    pub const unsubscribe_newHead: &str = "unsubscribe_newHead";
    pub const chain_newHead: &str = "chain_newHead";

    pub const chain_subscribeFinalizedHeads: &str = "chain_subscribeFinalizedHeads";
    pub const chain_subscribeFinalisedHeads: &str = "chain_subscribeFinalisedHeads";
    pub const chain_unsubscribeFinalizedHeads: &str = "chain_unsubscribeFinalizedHeads";
    pub const chain_unsubscribeFinalisedHeads: &str = "chain_unsubscribeFinalisedHeads";
    pub const chain_finalizedHead: &str = "chain_finalizedHead";

    pub const author_submitAndWatchExtrinsic: &str = "author_submitAndWatchExtrinsic";
    pub const author_unwatchExtrinsic: &str = "author_unwatchExtrinsic";
    pub const author_extrinsicUpdate: &str = "author_extrinsicUpdate";
}
