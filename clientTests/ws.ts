import WebSocket from "ws";
// import * as util from "util"


export const state_subscribeStorage = "state_subscribeStorage";
export const chain_subscribeAllHeads = "chain_subscribeAllHeads";
const CHAIN = "polkadot";

const client = new WebSocket("ws://localhost:9002");

client.on("message", (data) => {
    console.log(data.toString().slice(0, 100));
});

client.on("open", () => {
    for (let i = 0; i < 10; i++) {
        subscribeStorage(client, CHAIN);
    }
});

let _id = 0;

interface RpcRequest {
    id: string | number;
    chain: string;
    request: string;
}

class JsonRpcRequest {
    public readonly jsonrpc = "2.0";
    constructor(
        public readonly id: number,
        public readonly method: string,
        public readonly params: Record<string, object> | Array<object>,
    ) {
    }
}

function getCountId(): number {
    _id++;
    return _id;
}

export function subscribeStorage(client: WebSocket, chain: string): Promise<void> {
    return subscribe(client, chain, state_subscribeStorage);
}

export  function subscribeChain(client: WebSocket, chain: string): Promise<void> {
    return subscribe(client, chain, chain_subscribeAllHeads);
}

function subscribe(client: WebSocket, chain: string, method: string): Promise<void> {
    let req: RpcRequest = {
        id: "test",
        chain,
        request: JSON.stringify(new JsonRpcRequest(getCountId(), method, [])),
    }
    let jsonReq = JSON.stringify(req);

    return new Promise((res, rej) => {
        client.send(jsonReq, (err) => {
            if (err) {
                rej(err)
            } else {
                console.log(jsonReq);
                res()
            }
        })
    });
}
