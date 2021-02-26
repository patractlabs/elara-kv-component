pub(crate) mod dispatch;
pub(crate) mod handles;
pub(crate) mod rpc;
pub(crate) mod service;
pub(crate) mod session;

use std::collections::HashMap;

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::{message::MethodCall, session::Session};

// Note: we need the session to handle the method call
pub type MethodSender = UnboundedSender<(Session, MethodCall)>;
pub type MethodReceiver = UnboundedReceiver<(Session, MethodCall)>;

pub type MethodSenders = HashMap<&'static str, MethodSender>;
pub type MethodReceivers = HashMap<&'static str, MethodReceiver>;

#[allow(non_upper_case_globals)]
pub mod constants {
    pub const state_getRuntimeVersion: &str = "state_getRuntimeVersion";
    pub const system_health: &str = "system_health";

    pub const state_subscribeStorage: &str = "state_subscribeStorage";
    pub const state_unsubscribeStorage: &str = "state_unsubscribeStorage";
    pub const state_storage: &str = "state_storage";

    pub const state_subscribeRuntimeVersion: &str = "state_subscribeRuntimeVersion";
    pub const state_unsubscribeRuntimeVersion: &str = "state_unsubscribeRuntimeVersion";
    pub const state_runtimeVersion: &str = "state_runtimeVersion";

    pub const grandpa_subscribeJustifications: &str = "grandpa_subscribeJustifications";
    pub const grandpa_unsubscribeJustifications: &str = "grandpa_unsubscribeJustifications";
    pub const grandpa_justifications: &str = "grandpa_justifications";

    pub const chain_subscribeAllHeads: &str = "chain_subscribeAllHeads";
    pub const chain_unsubscribeAllHeads: &str = "chain_unsubscribeAllHeads";
    pub const chain_allHead: &str = "chain_allHead";

    pub const chain_subscribeNewHeads: &str = "chain_subscribeNewHeads";
    pub const chain_unsubscribeNewHeads: &str = "chain_unsubscribeNewHeads";
    pub const chain_newHead: &str = "chain_newHead";

    pub const chain_subscribeFinalizedHeads: &str = "chain_subscribeFinalizedHeads";
    pub const chain_unsubscribeFinalizedHeads: &str = "chain_unsubscribeFinalizedHeads";
    pub const chain_finalizedHead: &str = "chain_finalizedHead";

    pub const author_submitAndWatchExtrinsic: &str = "author_submitAndWatchExtrinsic";
    pub const author_unwatchExtrinsic: &str = "author_unwatchExtrinsic";
    pub const author_extrinsicUpdate: &str = "author_extrinsicUpdate";
}
