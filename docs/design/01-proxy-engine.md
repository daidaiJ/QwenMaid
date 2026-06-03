# 01 — 代理引擎 (Proxy Engine)

> axum HTTP 代理服务器，嵌入 Tauri 进程，拦截 Qwen Code 的所有 API 请求。

## 职责

- 监听本地端口（默认 18900），接收 Qwen Code 的 API 请求
- 根据请求的 endpoint + model 路由到对应的 Provider
- 执行鉴权转换和 API 格式转换
- 流式透传响应（SSE）
- 提取用量信息并记录到 SQLite

## 服务器生命周期

```
AgentBox 启动
  │
  ├─ 初始化 SQLite 数据库
  ├─ 加载 Provider/Model 配置
  ├─ 启动 axum HTTP 服务器 (localhost:18900)
  │     └─ 注册路由: /v1/chat/completions, /v1/messages, /v1/responses, /v1/models
  └─ 前端就绪

AgentBox 关闭
  │
  ├─ 停止接受新连接
  ├─ 等待进行中的请求完成（超时 30s）
  └─ 关闭 SQLite 连接
```

## 路由表

| 入站端点 | 说明 | 路由逻辑 |
|---|---|---|
| `/v1/chat/completions` | OpenAI Chat Completions | 查 model → 确定 provider/auth_type → 转发 |
| `/v1/messages` | Anthropic Messages | 查 model → 确定 provider → 鉴权转换 → 转发 |
| `/v1/responses` | OpenAI Responses | 查 model → 确定 provider → 转发 |
| `/v1/models` | 模型列表 | 合并所有 provider 的模型列表返回 |
| `/health` | 健康检查 | 返回代理状态 |

## 路由匹配算法

```
1. 解析请求体中的 model 字段
2. 在 models 表中查找 model_id 匹配的记录
3. 如果匹配到多个（不同 provider），使用 is_default=1 的那个
4. 如果没有 is_default，使用最近创建的
5. 如果完全没匹配，尝试直接透传到默认 provider
6. 根据 model 的 auth_type 选择处理管道
```

## 处理管道

```rust
// 伪代码
async fn handle_request(req: Request) -> Response {
    // 1. 解析请求
    let body = parse_body(req);
    let model_id = body.model;

    // 2. 路由匹配
    let route = db.find_model_route(&model_id)?;

    // 3. 鉴权转换
    let headers = auth::transform_headers(req.headers(), &route.auth_type, &route.api_key);

    // 4. API 格式转换（如需要）
    let transformed = transform::apply(body, route.auth_type)?;

    // 5. 上下文压缩（如启用）
    let compressed = compress::maybe_compress(transformed, config)?;

    // 6. 转发请求
    let upstream_req = build_upstream_request(route.base_url, route.endpoint, headers, compressed);
    let response = client.send(upstream_req).await?;

    // 7. 流式透传 + 用量提取
    if response.is_stream() {
        proxy_stream(response, |chunk| {
            usage::extract_from_chunk(chunk);  // 提取 usage 信息
            forward_to_client(chunk);          // 透传
        })
    } else {
        let usage = usage::extract_from_response(&response);
        db.record_request(usage);
        forward_to_client(response)
    }
}
```

## 错误处理

| 场景 | 处理方式 |
|---|---|
| Provider 不可达 | 返回 502 + 错误信息 |
| 鉴权失败 (401/403) | 透传 Provider 错误，记录日志 |
| 模型未找到 | 返回 404 + 可用模型列表提示 |
| 上游超时 | 返回 504 + 超时信息 |
| 请求格式错误 | 返回 400 + 格式说明 |

## 配置

代理服务器本身通过 AgentBox 的配置管理：

```json
{
  "proxy": {
    "port": 18900,
    "host": "127.0.0.1",
    "logLevel": "info",
    "maxRetries": 3,
    "timeoutMs": 120000,
    "contextCompression": {
      "enabled": false
    }
  }
}
```
