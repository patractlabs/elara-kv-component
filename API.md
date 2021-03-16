# Elara-KV API

## Config API

```json
{
    "id": hex_string,
    "compression": compression_type
}
```

- `id`： 客户端id
- `compression`: 由于异步性，推荐在初始化时设置。目前支持 `zlib`/`gzip`。设置后，后续`推送的订阅数据`改为通过`binary`推送相应的压缩格式，初始4字节为 `zlib`/`gzip`，后续跟着实际的压缩数据（为了保证一次解码） 。

TODO: 重新设计配置API


## Substrate 订阅相关API

订阅管理器提供的json API是基于封装substrate的jsonrpc格式作为一个子字段来进行处理。

substrate的jsonrpc格式参考 https://docs.elara.patract.io/

### 支持的订阅数据类型

- chain_allHead
- chain_newHead
- chain_finalizedHead
- state_runtimeVersion
- state_storage
- grandpa_justifications

### 请求

```json
{
    "id": hex_string,
    "chain": chain_name,
    "request": substrate_request_json
}
```

- `id`：16字节的hex id, 与订阅管理器通信的客户端id，客户端来保证唯一。
- `chain`: 区块链名，polkadot|kusama 等，基于elara-kv的配置项，根据相应的链名来实际订阅相关的链的数据。
- `request`: substrate原始的 jsonrpc 请求数据字符串。

示例:
```json
{
    "id": "b6c6d0aa16b0f5eb65e6fd87c6ffbba2",
    "chain": "polkadot",
    "request": "{\n\"id\": 141,\n\"jsonrpc\": \"2.0\",\n\"method\": \"state_subscribeStorage\",\n\"params\": [ [\"0x2aeddc77fe58c98d50bd37f1b90840f9cd7f37317cd20b61e9bd46fab87047149c21b6ab44c00eb3127a30e486492921e58f2564b36ab1ca21ff630672f0e76920edd601f8f2b89a\"]]}"
}
```

### 响应成功消息

```json
{
    "id": hex_string,
    "chain": chain_name,
    "result": substrate_result_json
}
```

在elara层的API没有错误时返回，不包含链层面的jsonrpc相关的问题

- `id`：16字节的hex id, 与订阅管理器通信的客户端id，与对应的请求的id相同
- `chain`: 区块链名，polkadot|kusama等
- `result`:  substrate原始的 jsonrpc 响应数据字符串，每个订阅响应可能该格式不同，比如state_subscribeStorage返回的是subscription id

示例:

```json
{
    "id": "b6c6d0aa16b0f5eb65e6fd87c6ffbba2",
    "chain": "polkadot",
    "result": "{\n\"jsonrpc\": \"2.0\",\n\"result\": \"0x91b171bb158e2d3848fa23a9f1c25182fb8e20313b2c1eb49219da7a70ce90c3\",\"id\": 1}"
}
```


### 响应错误消息

```json
{
    "id": hex_string,
    "chain": chain_name,
    "error": {
        "code": integer,
        "message": error_string
    }
}
```

在elara层的API有错误时返回

- `id`：16字节的hex id, 与订阅管理器通信的客户端id
- `chain`: 区块链名，polkadot|kusama|...
- `error`:  elara-kv请求有错误，比如chain 不存在，格式不为json等。

示例:

```json
{"id":"b6c6d0aa16b0f5eb65e6fd87c6ffbba2","chain":"po","error":{"code":-2,"message":"Chain not found"}}
```

### 推送订阅数据

订阅数据可以在配置API中设置为压缩格式。

```json
{
  "id": hex_string,
  "chain": chain_name,
  "data": substrate_subscription_data_json
}
```

- `id`：客户端id，与请求中的id对应
- `chain`: 区块链名，polkadot|kusama等
- `data`: substrate原始的 jsonrpc 订阅推送的数据

示例:
```json
{
    "id": "b6c6d0aa16b0f5eb65e6fd87c6ffbba2",
    "chain": "polkadot",
    "data": "{\"jsonrpc\": \"2.0\",\n\"method\":\"state_storage\", \n\"params\": {\n\"subscription\": \"ffMpMJgyQt3rmHx8\",\n\t\t\"result\": {\n\t\t  \"block\": \"0x04b67ec2b6ff34ebd58ed95fe9aad1068f805d2519ca8a24b986994b6764f410\",\n\t\t  \"changes\": [\n    [\"0x2aeddc77fe58c98d50bd37f1b90840f9cd7f37317cd20b61e9bd46fab870471456c62bce26605ee05c3c4c795e554a782e59ef5043ca9772f32dfb1ad7de832878d662194193955e\",              null ],[\"0x2aeddc77fe58c98d50bd37f1b90840f943a953ac082e08b6527ce262dbd4abf2e7731c5a045ae2174d185feff2d91e9a5c3c4c795e554a782e59ef5043ca9772f32dfb1ad7de832878d662194193955e\", \"0x3a875e45c13575f66eadb2d60608df9068a90e46ed33723098021e8cedd67d3a09f09f90ad20584949\"]]}}}"
}
```
