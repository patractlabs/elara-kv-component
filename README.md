# Elara kv subscribing component

## API

See <https://tower.im/teams/893726/repository_documents/806/>

## Config

```toml
[ws]
# 指定 elara-kv 的 ws server 地址
addr = "localhost:9002"

# 链节点相关的配置
# 如果配置某个节点x，则提供的订阅API里chain字段支持该链x的订阅
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
