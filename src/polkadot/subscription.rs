use async_jsonrpc_client::WsClientError;

use crate::substrate::dispatch::*;
use crate::{rpc_client::RpcClient, websocket::WsConnections};

pub async fn register_subscriptions(
    client: &RpcClient,
    conns: WsConnections,
) -> Result<(), WsClientError> {
    let chain = client.chain();
    let mut handler = DispatcherHandler::new();
    handler.register_dispatcher(Box::new(StateStorageDispatcher::new(chain)));
    handler.register_dispatcher(Box::new(StateRuntimeVersionDispatcher::new(chain)));
    handler.register_dispatcher(Box::new(ChainNewHeadDispatcher::new(chain)));
    handler.register_dispatcher(Box::new(ChainFinalizedHeadDispatcher::new(chain)));
    handler.register_dispatcher(Box::new(ChainAllHeadDispatcher::new(chain)));
    handler.register_dispatcher(Box::new(GrandpaJustificationDispatcher::new(chain)));

    handler.start_dispatch(client, conns).await
}
