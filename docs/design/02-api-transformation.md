# 02 — API 转换层 (API Transformation)

> 处理不同 LLM API 协议之间的鉴权映射和请求/响应格式转换。

## 鉴权转换

### Anthropic 鉴权映射

Qwen Code 发送 Anthropic 请求时使用 `Authorization: Bearer` 头，但标准 Anthropic API 需要 `x-api-key` 头。

```
入站:  Authorization: Bearer sk-ant-xxx
出站:  x-api-key: sk-ant-xxx
       anthropic-version: 2023-06-01
```

### 鉴权策略表

| auth_type | 入站头 | 出站头 | 说明 |
|---|---|---|---|
| `openai` | `Authorization: Bearer <key>` | `Authorization: Bearer <key>` | 透传 |
| `anthropic` | `Authorization: Bearer <key>` | `x-api-key: <key>` + `anthropic-version` | 映射转换 |
| `gemini` | `Authorization: Bearer <key>` | `x-goog-api-key: <key>` | 映射转换 |

### 实现

```rust
pub enum AuthStrategy {
    Passthrough,           // OpenAI: 直接透传 Bearer
    BearerToApiKey,        // Anthropic: Bearer → x-api-key
    BearerToGoogleKey,     // Gemini: Bearer → x-goog-api-key
}

pub fn transform_auth(headers: &HeaderMap, strategy: &AuthStrategy, api_key: &str) -> HeaderMap {
    match strategy {
        AuthStrategy::Passthrough => headers.clone(),
        AuthStrategy::BearerToApiKey => {
            let mut h = headers.clone();
            h.remove("authorization");
            h.insert("x-api-key", api_key.parse().unwrap());
            h.insert("anthropic-version", "2023-06-01".parse().unwrap());
            h
        }
        AuthStrategy::BearerToGoogleKey => {
            let mut h = headers.clone();
            h.remove("authorization");
            h.insert("x-goog-api-key", api_key.parse().unwrap());
            h
        }
    }
}
```

## Chat Completions → Responses 转换

Qwen Code 发送 `/v1/chat/completions` 格式，某些后端需要 `/v1/responses` 格式。

### 请求体转换

**Chat Completions 格式（入站）：**
```json
{
  "model": "gpt-4o",
  "messages": [
    { "role": "system", "content": "You are helpful." },
    { "role": "user", "content": "Hello" }
  ],
  "temperature": 0.7,
  "max_tokens": 4096,
  "stream": true
}
```

**Responses 格式（出站）：**
```json
{
  "model": "gpt-4o",
  "input": [
    { "role": "system", "content": "You are helpful." },
    { "role": "user", "content": "Hello" }
  ],
  "temperature": 0.7,
  "max_output_tokens": 4096,
  "stream": true
}
```

### 字段映射

| Chat Completions | Responses | 说明 |
|---|---|---|
| `messages` | `input` | 消息数组重命名 |
| `max_tokens` | `max_output_tokens` | 参数重命名 |
| `temperature` | `temperature` | 透传 |
| `top_p` | `top_p` | 透传 |
| `stream` | `stream` | 透传 |
| `tools` | `tools` | 格式略有差异，需适配 |
| `tool_choice` | `tool_choice` | 格式略有差异 |
| `response_format` | `text.format` | 嵌套层级变化 |

### 响应格式转换

**Chat Completions 响应（非流式）：**
```json
{
  "choices": [
    {
      "message": { "role": "assistant", "content": "Hi!" },
      "finish_reason": "stop"
    }
  ],
  "usage": {
    "prompt_tokens": 10,
    "completion_tokens": 5,
    "total_tokens": 15
  }
}
```

**Responses 响应（非流式）：**
```json
{
  "output": [
    {
      "type": "message",
      "content": [{ "type": "output_text", "text": "Hi!" }]
    }
  ],
  "usage": {
    "input_tokens": 10,
    "output_tokens": 5,
    "total_tokens": 15
  }
}
```

### 流式事件转换

**Chat Completions SSE：**
```
data: {"choices":[{"delta":{"content":"Hi"}}]}
data: {"choices":[{"delta":{"content":"!"}}]}
data: [DONE]
```

**Responses SSE：**
```
data: {"type":"response.output_text.delta","delta":"Hi"}
data: {"type":"response.output_text.delta","delta":"!""}
data: {"type":"response.completed"}
```

### 实现策略

```rust
pub struct TransformPipeline {
    direction: TransformDirection,
}

pub enum TransformDirection {
    ChatToResponses,     // /v1/chat/completions → /v1/responses
    ResponsesToChat,     // /v1/responses → /v1/chat/completions（反向兼容）
    Passthrough,         // 无需转换
}

impl TransformPipeline {
    pub fn transform_request(&self, body: Value) -> Result<Value> { ... }
    pub fn transform_response_chunk(&self, chunk: Value) -> Result<Value> { ... }
}
```

## 路由决策矩阵

> Qwen Code 原生支持 OpenAI / Anthropic / Gemini 三种 API 协议，Gemini 请求可直接透传。

| 入站端点 | target auth_type | 动作 |
|---|---|---|
| `/v1/chat/completions` | `openai` | 鉴权透传，body 透传 |
| `/v1/chat/completions` | `anthropic` | **鉴权转换**，body 按需转换为 messages 格式 |
| `/v1/messages` | `anthropic` | **鉴权转换**，body 透传 |
| `/v1/responses` | `openai` | 鉴权透传，body 透传 |
| Gemini 原生端点 | `gemini` | 全部透传（鉴权 + body 均透传） |

**Gemini 特殊说明：** Qwen Code 使用 `@google/genai` SDK 直接与 Gemini API 通信，请求格式已经是原生 Gemini 格式。代理层只需透传鉴权头和请求体，不做任何转换。唯一的处理是从 `Authorization: Bearer` 中提取 key 注入到 `x-goog-api-key` 头（如果 Gemini provider 需要 API Key 认证而非 OAuth）。
