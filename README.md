# Elara kv subscribing component

## API

See <https://tower.im/teams/893726/repository_documents/806/>

## Config

```toml
[ws]
# 指定 elara-kv 的 ws server 地址
addr = "localhost:9002"

# 链节点相关的配置，目前只支持 `polkadot` 和 `kusama`
[nodes.polkadot]
# 配置 polkadot 的节点地址
url = "wss://rpc.polkadot.io"
[nodes.kusama]
url = "wss://kusama-rpc.polkadot.io"
```

## Start

该项目使用 `env_logger` 打印 log:

```bash
RUST_LOG=info elara-kv --config path/to/config.toml
```
